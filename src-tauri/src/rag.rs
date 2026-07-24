use crate::config::RagConfig;
use crate::db::{Ability, Character, Place, Relation};
use crate::error::{AppError, AppResult};
use crate::providers::{self, ChatMessage};
use crate::vector_store::{self, Chunk};
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub doc: String,
    /// Short preview centered on the part of the chunk that matched the query.
    pub quote: String,
    /// The full retrieved passage, shown in the citation modal.
    pub text: String,
    /// The `[Fonte N]` number this passage was given in the prompt, so the UI can
    /// map an inline `[N]` marker in the answer back to its source.
    pub mark: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub text: String,
    pub sources: Vec<Source>,
}

/// Metadata carried alongside freshly built chunks before persistence.
pub struct BuiltDoc {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub chunks: Vec<Chunk>,
}

// ---- Chunking -------------------------------------------------------------

/// Split raw text into overlapping chunks. `chunk_size`/`overlap` are approx
/// tokens (~4 chars/token), split on word boundaries so chunks read as quotes.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let target_chars = chunk_size.max(50) * 4;
    let overlap_chars = overlap.min(chunk_size / 2) * 4;

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < words.len() {
        let mut end = start;
        let mut len = 0usize;
        while end < words.len() && len < target_chars {
            len += words[end].len() + 1;
            end += 1;
        }
        chunks.push(words[start..end].join(" "));
        if end >= words.len() {
            break;
        }
        let mut back = 0usize;
        let mut idx = end;
        while idx > start && back < overlap_chars {
            idx -= 1;
            back += words[idx].len() + 1;
        }
        start = idx.max(start + 1);
    }
    chunks
}

fn file_kind(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => ext.to_uppercase(),
        _ => "TXT".into(),
    }
}

/// A ~200-char preview of `text`, centered on the earliest query term that
/// appears in the chunk — so the quote shows *why* the passage was retrieved,
/// not just its opening words. Falls back to the start when nothing matches.
fn snippet(text: &str, question: &str) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    let n = chars.len();
    if n == 0 {
        return String::new();
    }
    let lower: String = text.to_lowercase();

    // Query terms worth matching (skip short stop-ish words).
    let terms: Vec<String> = question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 3)
        .map(|w| w.to_string())
        .collect();

    // Earliest byte match of any term → convert to a char index.
    let hit_byte = terms.iter().filter_map(|t| lower.find(t.as_str())).min();
    let center = match hit_byte {
        Some(b) => lower[..b].chars().count(),
        None => 0,
    };

    const WIN: usize = 200;
    let start = center.saturating_sub(WIN / 3);
    let end = (start + WIN).min(n);
    let start = end.saturating_sub(WIN).min(start);

    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(chars[start..end].iter());
    if end < n {
        out.push('…');
    }
    format!("\"{}\"", out.trim())
}

// ---- Ingestion ------------------------------------------------------------

/// Chunk + embed a document. The caller persists the result into the DB.
pub async fn build_document(
    client: &reqwest::Client,
    cfg: &RagConfig,
    name: String,
    content: String,
) -> AppResult<BuiltDoc> {
    let pieces = chunk_text(&content, cfg.chunk_size, cfg.chunk_overlap);
    // Content-addressed id: re-ingesting identical bytes yields the same id, so
    // the document row is replaced in place rather than duplicated.
    let doc_id = blake3::hash(content.as_bytes()).to_hex().to_string();
    let vectors = if pieces.is_empty() {
        Vec::new()
    } else {
        providers::embed(client, cfg, &pieces).await?
    };
    let chunks = pieces
        .into_iter()
        .zip(vectors)
        .enumerate()
        .map(|(i, (text, vector))| Chunk {
            id: Uuid::new_v4().to_string(),
            doc_id: doc_id.clone(),
            doc_name: name.clone(),
            ordinal: i,
            text,
            vector,
        })
        .collect();
    Ok(BuiltDoc { kind: file_kind(&name), id: doc_id, name, chunks })
}

// ---- Ask (RAG-first) ------------------------------------------------------

/// One prior turn of the conversation, sent from the frontend so the LLM keeps
/// context across the whole chat.
#[derive(Debug, Clone, Deserialize)]
pub struct HistoryTurn {
    pub role: String, // "user" | "assistant"
    pub text: String,
}

/// Retrieve context for a question and build the full chat message list
/// (system + history + user), returning it alongside the deduped sources.
async fn build_chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
    question: String,
    history: Vec<HistoryTurn>,
    entity_names: &[String],
    relations: &[Relation],
) -> AppResult<(Vec<ChatMessage>, Vec<Source>)> {
    let mut sources: Vec<Source> = Vec::new();
    let mut context = String::new();

    if !chunks.is_empty() {
        // If the question names a specific chapter/document ("capítulo 1", "cap. 2"),
        // pull THAT document's own chunks in reading order instead of a semantic mix
        // that could surface other chapters. Retrieving the wrong chapter is what
        // makes the model spiral ("this is chapter 3, not 1…") and truncate its
        // answer; giving it the right document removes the confusion outright.
        let chapters = referenced_chapters(&question);
        let target_docs = if chapters.is_empty() {
            std::collections::HashSet::new()
        } else {
            docs_for_chapters(&chapters, chunks)
        };

        let hits: Vec<vector_store::ScoredChunk> = if !target_docs.is_empty() {
            let mut selected: Vec<&Chunk> =
                chunks.iter().filter(|c| target_docs.contains(&c.doc_id)).collect();
            selected.sort_by(|a, b| {
                cmp_doc(&a.doc_name, &b.doc_name).then(a.ordinal.cmp(&b.ordinal))
            });
            // Budget the material so a long chapter can't blow up the context window
            // (which itself causes truncated answers on reasoning models).
            const TARGET_DOC_BUDGET: usize = 14_000;
            let mut used = 0usize;
            let mut out = Vec::new();
            for c in selected {
                used += c.text.len();
                out.push(vector_store::ScoredChunk { chunk: c.clone(), score: 1.0 });
                if used >= TARGET_DOC_BUDGET {
                    break;
                }
            }
            out
        } else {
            let qvec = providers::embed(client, cfg, &[question.clone()]).await?;
            let mut hits = vector_store::search(chunks, &qvec[0], cfg.top_k);

            // Hybrid retrieval: add lexical matches the embedding model under-ranked.
            // A question like "onde caiu o meteorito?" must surface the chunk with the
            // literal word "meteorito" even if cosine ranked it outside top-k.
            let have: std::collections::HashSet<String> =
                hits.iter().map(|h| h.chunk.id.clone()).collect();
            let lexical: Vec<vector_store::ScoredChunk> =
                vector_store::keyword_search(chunks, &question, cfg.top_k)
                    .into_iter()
                    .filter(|h| !have.contains(&h.chunk.id))
                    .collect();
            hits.extend(lexical);

            // Positional questions ("primeira frase", "como começa") are about a
            // document's opening, which semantic search rarely ranks near the query.
            // Force-inject every document's opening chunk (ordinal 0) so the model has
            // the real start to work from; reading-order labels let it pick the doc
            // whose name matches the chapter asked.
            if wants_opening(&question) {
                let have: std::collections::HashSet<&str> =
                    hits.iter().map(|h| h.chunk.id.as_str()).collect();
                let openings: Vec<vector_store::ScoredChunk> = chunks
                    .iter()
                    .filter(|c| c.ordinal == 0 && !have.contains(c.id.as_str()))
                    .map(|c| vector_store::ScoredChunk { chunk: c.clone(), score: 1.0 })
                    .collect();
                hits.extend(openings);
            }

            // First-occurrence questions ("o que X falou pela PRIMEIRA VEZ para Y?",
            // "quando se conheceram?") are about the EARLIEST scene in reading order.
            // Cosine similarity has no notion of "earliest", so top-k lands on whatever
            // late chapter phrases it best — the model then answers "não foi possível
            // identificar" (or invents a prior acquaintance). Force-inject the earliest
            // chunks that actually mention the entities named in the question.
            if wants_first_time(&question) {
                let have: std::collections::HashSet<&str> =
                    hits.iter().map(|h| h.chunk.id.as_str()).collect();
                let firsts: Vec<vector_store::ScoredChunk> =
                    first_mention_chunks(&question, entity_names, chunks, 5)
                        .into_iter()
                        .filter(|c| !have.contains(c.id.as_str()))
                        .map(|c| vector_store::ScoredChunk { chunk: c.clone(), score: 1.0 })
                        .collect();
                hits.extend(firsts);
            }

            // Optional rerank: one cheap LLM pass orders the retrieved chunks by
            // relevance to the question, trimming top-k noise before we build the
            // context. Non-fatal — on any failure the hybrid order is kept.
            if cfg.rerank {
                hits = rerank_hits(client, cfg, &question, hits).await;
            }
            hits
        };

        // Context is re-sorted into reading order, grouped by document, and each
        // fragment is labeled with its position — so the model can answer
        // positional questions ("primeira frase", "início do capítulo") and never
        // confuses fragments of one document with another's.
        let mut ordered: Vec<&vector_store::ScoredChunk> = hits.iter().collect();
        // Natural (number-aware) document order: "Capítulo 2" must come before
        // "Capítulo 10". Plain string compare puts 10 first, which silently breaks
        // reading order — and with it every "first time / earliest" question.
        ordered.sort_by(|a, b| {
            cmp_doc(&a.chunk.doc_name, &b.chunk.doc_name)
                .then(a.chunk.ordinal.cmp(&b.chunk.ordinal))
        });
        // Number each fragment [Fonte N] in the exact order the model reads it, so
        // the model can declare which it used with matching [N] markers. `sources`
        // is built in the SAME order → sources[N-1] is fragment N.
        for (i, hit) in ordered.iter().enumerate() {
            let n = i + 1;
            sources.push(Source {
                doc: hit.chunk.doc_name.clone(),
                quote: snippet(&hit.chunk.text, &question),
                text: hit.chunk.text.trim().to_string(),
                mark: n,
            });
            // Only the opening is a meaningful, stable landmark. Do NOT expose the
            // raw chunk index — it's an internal ~200-word slice, not a document
            // section, and leaking the number makes the model (and user) treat
            // "trecho 4" as real structure. Reading order is conveyed by position
            // in the prompt.
            let label = if hit.chunk.ordinal == 0 {
                format!("{} · início do documento", hit.chunk.doc_name)
            } else {
                hit.chunk.doc_name.clone()
            };
            context.push_str(&format!("[Fonte {}] [Documento: {}]\n{}\n\n", n, label, hit.chunk.text));
        }
    }

    let context_block = if context.is_empty() {
        "(nenhum documento indexado neste vault ou nenhum trecho relevante)".to_string()
    } else {
        context
    };

    // GraphRAG-lite: if the question names known entities, pull their relation
    // subgraph (edited/added by the user when applicable) and hand the model the
    // structured facts. Chunk retrieval is cosine-blind to relations — the graph
    // answers multi-hop questions ("quem é o mestre de Victor?") the prose misses.
    let graph_block = graph_context(&question, entity_names, relations);

    let system = format!(
        "{}\n\n{}Cada trecho abaixo vem rotulado com [Fonte N] [Documento: nome]. Os trechos de um \
mesmo documento aparecem em ordem de leitura; \"início do documento\" marca a abertura \
do texto. [Fonte N] é só um identificador para citação — NÃO é seção ou capítulo \
numerado, então não invente nem cite \"trecho N\". Ao responder sobre um documento \
específico (ex.: \"capítulo 1\"), use SOMENTE os trechos cujo nome de documento \
corresponde — nunca misture documentos diferentes. Para perguntas sobre a abertura \
(\"primeira frase\", \"como começa\"), use o trecho marcado \"início do documento\" \
daquele documento; se ele não estiver presente, diga que não recuperou a abertura.\n\n\
Responda SOMENTE com o que está escrito nos trechos. NÃO deduza, NÃO suponha e NÃO preencha \
lacunas: se o texto não afirma algo (por exemplo, que dois personagens já se conheciam antes), \
não afirme — e não trate uma suposição sua como fato. Se a informação pedida não estiver nos \
trechos, diga isso e aponte o que faltou. Para perguntas de PRIMEIRA ocorrência (\"primeira vez \
que…\", \"quando se conheceram\", \"primeira coisa que disse\"), os documentos vêm em ordem de \
capítulo: use a ocorrência mais ANTIGA presente nos trechos, cite-a, e nunca escolha uma cena \
posterior. Se houver diálogo literal, transcreva a fala entre aspas.\n\n\
Ao final de cada afirmação apoiada nos trechos, cite a(s) fonte(s) usada(s) com o marcador \
correspondente, ex.: [1] ou [2][3]. Cite APENAS as fontes que realmente sustentam a resposta. \
Escreva de forma fluida: evite repetir o nome próprio dos personagens a cada frase — use pronomes \
ou referências quando o sujeito já estiver claro.\n\n\
Contexto recuperado da base de conhecimento:\n{}",
        cfg.system_prompt, graph_block, context_block
    );
    let mut messages = vec![ChatMessage { role: "system", content: system }];
    for turn in history {
        let role = if turn.role == "assistant" { "assistant" } else { "user" };
        messages.push(ChatMessage { role, content: turn.text });
    }
    messages.push(ChatMessage { role: "user", content: question });

    // NOTE: sources are returned in [Fonte N] order and NOT deduped here — the
    // caller filters to what the answer used (declared markers or overlap) and
    // dedupes afterward, so index N stays aligned with the prompt.
    Ok((messages, sources))
}

pub async fn ask(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
    question: String,
    history: Vec<HistoryTurn>,
    entity_names: &[String],
    relations: &[Relation],
) -> AppResult<Answer> {
    // Corrective RAG: draft, grade, and (if the draft falls short) re-retrieve
    // wider and answer once more. Bounded to one retry.
    if cfg.corrective {
        let (msgs1, src1) = build_chat(
            client, cfg, chunks, question.clone(), history.clone(), entity_names, relations,
        )
        .await?;
        let draft = providers::chat_internal(client, cfg, &msgs1, &cfg.llm_model).await?;
        if grade_answer(client, cfg, &question, &draft).await.unwrap_or(true) {
            let text = providers::chat(client, cfg, &msgs1).await?;
            let sources = cited_sources(src1, &strip_think(&text));
            return Ok(Answer { text, sources });
        }
        let cfg2 = widen(cfg);
        let (msgs2, src2) =
            build_chat(client, &cfg2, chunks, question, history, entity_names, relations).await?;
        let text = providers::chat(client, &cfg2, &msgs2).await?;
        let sources = cited_sources(src2, &strip_think(&text));
        return Ok(Answer { text, sources });
    }

    let (messages, sources) =
        build_chat(client, cfg, chunks, question, history, entity_names, relations).await?;
    let text = providers::chat(client, cfg, &messages).await?;
    let sources = cited_sources(sources, &strip_think(&text));
    Ok(Answer { text, sources })
}

/// Streaming variant of [`ask`]: `on_token` fires per generated text delta.
/// The sources are returned once retrieval + generation complete.
pub async fn ask_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
    question: String,
    history: Vec<HistoryTurn>,
    entity_names: &[String],
    relations: &[Relation],
    cancel: &std::sync::atomic::AtomicBool,
    on_token: F,
) -> AppResult<Vec<Source>> {
    // Corrective RAG: a cheap non-streamed draft (internal, no thinking) is graded
    // first to decide whether to widen retrieval. The FINAL answer is ALWAYS
    // streamed live — the draft is a check, never what the user sees. Bounded to
    // one retry, no open loop.
    if cfg.corrective {
        let (msgs1, src1) = build_chat(
            client, cfg, chunks, question.clone(), history.clone(), entity_names, relations,
        )
        .await?;
        let draft = providers::chat_internal(client, cfg, &msgs1, &cfg.llm_model).await?;
        if grade_answer(client, cfg, &question, &draft).await.unwrap_or(true) {
            let answer =
                providers::chat_stream(client, cfg, &msgs1, cfg.show_thinking, cancel, on_token).await?;
            return Ok(cited_sources(src1, &strip_think(&answer)));
        }
        let cfg2 = widen(cfg);
        let (msgs2, src2) =
            build_chat(client, &cfg2, chunks, question, history, entity_names, relations).await?;
        let answer =
            providers::chat_stream(client, &cfg2, &msgs2, cfg.show_thinking, cancel, on_token).await?;
        return Ok(cited_sources(src2, &strip_think(&answer)));
    }

    let (messages, sources) =
        build_chat(client, cfg, chunks, question, history, entity_names, relations).await?;
    let answer =
        providers::chat_stream(client, cfg, &messages, cfg.show_thinking, cancel, on_token).await?;
    // Cite only what the answer actually drew on: prefer the [N] markers the model
    // declared, falling back to content overlap when it emitted none.
    Ok(cited_sources(sources, &strip_think(&answer)))
}

/// Generate a short session title (≈3–6 words) summarizing the topic, from the
/// first user message and the assistant's answer. No quotes, no trailing period.
pub async fn summarize_title(
    client: &reqwest::Client,
    cfg: &RagConfig,
    question: &str,
    answer: &str,
) -> AppResult<String> {
    let system = "Gere um TÍTULO curto (no máximo 6 palavras) que resuma o assunto da conversa. \
Responda só o título, no idioma da conversa, sem aspas, sem markdown, sem ponto final. \
Seja específico e conciso, como um título de conversa de chat. \
Não raciocine em voz alta nem inclua qualquer texto além do título. /no_think";
    let mut ans = answer.to_string();
    ans.truncate(800);
    let user = format!("Pergunta do usuário:\n{question}\n\nResposta:\n{ans}\n\nTítulo:");
    let messages = vec![
        ChatMessage { role: "system", content: system.to_string() },
        ChatMessage { role: "user", content: user },
    ];
    let raw = providers::chat_internal(client, cfg, &messages, &cfg.llm_model).await?;
    // Reasoning models emit a <think>…</think> block first — drop it so the
    // title isn't literally "<think>".
    let raw = strip_think(&raw);
    // Take the first non-empty line, strip surrounding quotes / trailing punctuation.
    let mut t = raw
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'' || c == '«' || c == '»' || c == '.')
        .trim()
        .to_string();
    // Strip a leading "Título:" the model may echo.
    if let Some(rest) = t.strip_prefix("Título:").or_else(|| t.strip_prefix("Titulo:")) {
        t = rest.trim().to_string();
    }
    let chars: String = t.chars().take(60).collect();
    Ok(chars)
}

/// Meaningful lowercased content words (len ≥ 4, minus common PT stop words),
/// used to measure overlap between the answer and each candidate source.
fn content_terms(text: &str) -> std::collections::HashSet<String> {
    const STOP: [&str; 40] = [
        "para", "pelo", "pela", "pelos", "pelas", "dos", "das", "com", "sem", "que",
        "uma", "uns", "umas", "essa", "esse", "esses", "essas", "isso", "aquilo",
        "sobre", "esta", "este", "estes", "estas", "seu", "sua", "seus", "suas",
        "onde", "qual", "quais", "quem", "como", "quando", "porque", "mas", "por",
        "nao", "não", "mais",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Keep only the sources the generated answer actually used: those sharing at
/// least one content word with it. A "não encontrei" answer (no content terms)
/// cites nothing. Result is ordered by overlap strength (strongest first).
fn relevant_sources(sources: Vec<Source>, answer: &str) -> Vec<Source> {
    let terms = content_terms(answer);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, Source)> = sources
        .into_iter()
        .map(|s| {
            let overlap = content_terms(&s.text).intersection(&terms).count();
            (overlap, s)
        })
        .filter(|(overlap, _)| *overlap > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, s)| s).collect()
}

/// GraphRAG-lite context. Detects which known entities the question names, then
/// collects the relation subgraph around them (the seeds' own edges plus one hop
/// to their neighbors' edges) and renders it as a structured `Fato → Fato` block.
/// Empty string when the question names no known entity or no relation touches it,
/// so the caller can inject it unconditionally. Capped so a hub node can't flood
/// the prompt.
fn graph_context(question: &str, entity_names: &[String], relations: &[Relation]) -> String {
    if entity_names.is_empty() || relations.is_empty() {
        return String::new();
    }
    let q = question.to_lowercase();

    // Seeds: known entity names that appear in the question. Longer names first so
    // "Cesar Magnus" is preferred over "Cesar" when both would match.
    let mut names: Vec<&String> = entity_names.iter().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut seeds: std::collections::HashSet<String> = Default::default();
    for name in names {
        let low = name.to_lowercase();
        if low.chars().count() >= 3 && q.contains(&low) {
            seeds.insert(low);
        }
    }
    if seeds.is_empty() {
        return String::new();
    }

    // Expand one hop: any node directly related to a seed joins the frontier, so
    // we capture "mentor do mestre de X" style chains without pulling the whole map.
    let touches = |r: &Relation, set: &std::collections::HashSet<String>| {
        set.contains(&r.from.to_lowercase()) || set.contains(&r.to.to_lowercase())
    };
    let mut frontier = seeds.clone();
    for r in relations {
        if touches(r, &seeds) {
            frontier.insert(r.from.to_lowercase());
            frontier.insert(r.to.to_lowercase());
        }
    }

    // Collect edges among the frontier, deduped, capped.
    const MAX_EDGES: usize = 40;
    let mut seen: std::collections::HashSet<(String, String, String)> = Default::default();
    let mut lines: Vec<String> = Vec::new();
    for r in relations {
        if lines.len() >= MAX_EDGES {
            break;
        }
        let fl = r.from.to_lowercase();
        let tl = r.to.to_lowercase();
        if !(frontier.contains(&fl) || frontier.contains(&tl)) {
            continue;
        }
        let key = (fl, tl, r.label.to_lowercase());
        if !seen.insert(key) {
            continue;
        }
        let label = if r.label.trim().is_empty() { "relaciona-se com" } else { r.label.trim() };
        lines.push(format!("- {} —({})→ {}", r.from, label, r.to));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "Relações conhecidas do grafo de entidades (fatos estruturados, curados pelo \
usuário quando aplicável — trate como verdade sobre quem se relaciona com quem):\n{}\n\n",
        lines.join("\n")
    )
}

/// Final citation set for an answer. Prefers the explicit `[N]` markers the model
/// declared (mapped to the `[Fonte N]` fragments), and only when it declared none
/// falls back to the content-overlap heuristic. Deduped by document + quote.
fn cited_sources(sources: Vec<Source>, answer: &str) -> Vec<Source> {
    let marks = parse_markers(answer, sources.len());
    let picked: Vec<Source> = if marks.is_empty() {
        relevant_sources(sources, answer)
    } else {
        marks
            .into_iter()
            .filter_map(|n| sources.get(n - 1).cloned())
            .collect()
    };
    let mut seen: std::collections::HashSet<(String, String)> = Default::default();
    picked
        .into_iter()
        .filter(|s| seen.insert((s.doc.clone(), s.quote.clone())))
        .collect()
}

/// A copy of the config with a wider retrieval net, used for the corrective
/// retry: more chunks (capped) so the second answer sees context the first missed.
fn widen(cfg: &RagConfig) -> RagConfig {
    let mut c = cfg.clone();
    c.top_k = (cfg.top_k * 2).clamp(cfg.top_k, 12);
    c
}

/// Grade whether `answer` actually resolves `question` (Corrective RAG). Returns
/// true = adequate. The model replies with a tiny JSON verdict; parse/other
/// failures default to adequate so grading never blocks a usable answer.
async fn grade_answer(
    client: &reqwest::Client,
    cfg: &RagConfig,
    question: &str,
    answer: &str,
) -> AppResult<bool> {
    // An explicit "não encontrei" is a valid, complete answer — don't force a retry
    // that would only re-confirm the absence.
    let system = "Você avalia se uma RESPOSTA resolve de fato a PERGUNTA do usuário, com base \
em uma base de conhecimento. Responda APENAS com JSON, sem markdown: {\"adequate\":true|false}. \
adequate=false só quando a resposta é claramente incompleta, evasiva ou não endereça a pergunta. \
Uma resposta que afirma honestamente não ter encontrado a informação nos documentos é adequate=true. \
/no_think";
    let mut ans = strip_think(answer);
    ans.truncate(2000);
    let user = format!("PERGUNTA:\n{question}\n\nRESPOSTA:\n{ans}");
    let messages = vec![
        ChatMessage { role: "system", content: system.to_string() },
        ChatMessage { role: "user", content: user },
    ];
    let raw = providers::chat_internal(client, cfg, &messages, &cfg.llm_model).await?;
    let raw = strip_think(&raw);
    let json = extract_json_block(&raw).ok_or_else(|| AppError::Msg("grade: JSON inválido".into()))?;
    #[derive(Deserialize)]
    struct Verdict {
        #[serde(default)]
        adequate: bool,
    }
    let v: Verdict = serde_json::from_str(&json).map_err(|e| AppError::Msg(format!("grade: {e}")))?;
    Ok(v.adequate)
}

/// Distinct `[N]` citation markers in first-seen order, keeping only numbers in
/// `1..=max` so stray brackets in prose (e.g. "[sic]", "[2023]") are ignored.
fn parse_markers(text: &str, max: usize) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut out: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            let mut num = 0usize;
            let mut any = false;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                num = num * 10 + (bytes[j] - b'0') as usize;
                any = true;
                j += 1;
            }
            if any && j < bytes.len() && bytes[j] == b']' && num >= 1 && num <= max {
                if !out.contains(&num) {
                    out.push(num);
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Cheap LLM rerank: ask the model to order the retrieved chunks by relevance to
/// the question and reorder accordingly. Any index the model omits is appended in
/// its original position, so no chunk is silently dropped. Non-fatal.
async fn rerank_hits(
    client: &reqwest::Client,
    cfg: &RagConfig,
    question: &str,
    hits: Vec<vector_store::ScoredChunk>,
) -> Vec<vector_store::ScoredChunk> {
    if hits.len() <= 1 {
        return hits;
    }
    let list = hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let t: String = h.chunk.text.chars().take(400).collect();
            format!("[{i}] {t}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let system = "Você ordena trechos por relevância a uma pergunta. Responda APENAS \
com JSON, sem markdown: {\"order\":[índices do mais relevante ao menos relevante]}. \
Use somente os índices fornecidos, sem repetir. /no_think";
    let user = format!("Pergunta: {question}\n\nTrechos:\n{list}");
    let messages = vec![
        ChatMessage { role: "system", content: system.to_string() },
        ChatMessage { role: "user", content: user },
    ];
    let raw = match providers::chat_internal(client, cfg, &messages, &cfg.llm_model).await {
        Ok(r) => r,
        Err(_) => return hits,
    };
    let raw = strip_think(&raw);
    let json = match extract_json_block(&raw) {
        Some(j) => j,
        None => return hits,
    };
    #[derive(Deserialize)]
    struct Order {
        #[serde(default)]
        order: Vec<usize>,
    }
    let parsed: Order = match serde_json::from_str(&json) {
        Ok(p) => p,
        Err(_) => return hits,
    };
    if parsed.order.is_empty() {
        return hits;
    }
    let mut slots: Vec<Option<vector_store::ScoredChunk>> = hits.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(slots.len());
    for idx in parsed.order {
        if let Some(slot) = slots.get_mut(idx) {
            if let Some(h) = slot.take() {
                out.push(h);
            }
        }
    }
    for slot in slots.into_iter().flatten() {
        out.push(slot);
    }
    out
}

/// Chapter numbers explicitly named in a phrase ("capítulo 1", "cap. 2", "cap 3").
/// Scans for a token starting with "cap" followed (within two tokens) by a number.
fn referenced_chapters(text: &str) -> Vec<u32> {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let mut out = Vec::new();
    for i in 0..tokens.len() {
        let tok = tokens[i];
        // Only real chapter markers: "cap", "capitulo"/"capítulo" (any accent),
        // optionally glued to the number ("cap2", "capitulo3"). Avoids "capaz".
        let letters: String = tok.chars().take_while(|c| c.is_alphabetic()).collect();
        let is_marker = letters == "cap" || letters.starts_with("capitul") || letters.starts_with("capítul");
        if !is_marker {
            continue;
        }
        // "cap1"/"capitulo1" glued to the number...
        let rest: String = tok.trim_start_matches(char::is_alphabetic).to_string();
        if let Ok(n) = rest.parse::<u32>() {
            out.push(n);
            continue;
        }
        // ...or a numeric token within the next two.
        for tok in tokens.iter().skip(i + 1).take(2) {
            if let Ok(n) = tok.parse::<u32>() {
                out.push(n);
                break;
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Document ids whose name references one of the given chapter numbers.
fn docs_for_chapters(
    chapters: &[u32],
    chunks: &[vector_store::Chunk],
) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for c in chunks {
        let in_name = referenced_chapters(&c.doc_name);
        if chapters.iter().any(|ch| in_name.contains(ch)) {
            ids.insert(c.doc_id.clone());
        }
    }
    ids
}

/// Heuristic: does the question target a position near the opening of a text?
/// These need the document's first chunk (to read/count from), which semantic
/// retrieval alone rarely surfaces. Covers "primeira frase", "segunda frase",
/// "terceiro parágrafo", "como começa", "início do capítulo", etc.
fn wants_opening(question: &str) -> bool {
    let q = question.to_lowercase();

    // Direct opening cues — fire on their own.
    const DIRECT: [&str; 6] = ["como começa", "como comeca", "início do", "inicio do", "abertura", "começo do"];
    if DIRECT.iter().any(|c| q.contains(c)) {
        return true;
    }

    // A textual unit ("frase"/"linha"/"parágrafo"/"palavra") combined with a low
    // ordinal ("primeir…"/"segund…"/"terceir…") is a count-from-the-start query.
    let has_unit = ["frase", "linha", "parágrafo", "paragrafo", "palavra"]
        .iter()
        .any(|u| q.contains(u));
    let has_low_ordinal = ["primeir", "segund", "terceir", "quart", "quint"]
        .iter()
        .any(|o| q.contains(o));
    has_unit && has_low_ordinal
}

/// Lowercase + strip Portuguese accents + collapse whitespace. Used for all
/// text matching (grounding, entity lookup) so "Salazar" matches "SALAZÁR".
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        let c = match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            other => other,
        };
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            for l in c.to_lowercase() {
                out.push(l);
            }
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Natural (number-aware) comparison of document names, so "Capítulo 2" sorts
/// before "Capítulo 10". Plain lexicographic compare inverts them, which destroys
/// reading order on any work with 10+ chapters.
fn cmp_doc(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (fa, fb) = (fold(a), fold(b));
    let mut ia = fa.chars().peekable();
    let mut ib = fb.chars().peekable();
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    // Compare whole numeric runs by value, not digit by digit.
                    let na: String = std::iter::from_fn(|| ia.next_if(|c| c.is_ascii_digit())).collect();
                    let nb: String = std::iter::from_fn(|| ib.next_if(|c| c.is_ascii_digit())).collect();
                    let va = na.trim_start_matches('0');
                    let vb = nb.trim_start_matches('0');
                    let ord = va.len().cmp(&vb.len()).then_with(|| va.cmp(vb));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    ia.next();
                    ib.next();
                    let ord = ca.cmp(&cb);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
        }
    }
}

/// Heuristic: is the question about the FIRST occurrence of something (first
/// meeting, first words, first appearance)? These need the earliest chunk in
/// reading order, which cosine similarity never ranks for.
fn wants_first_time(question: &str) -> bool {
    let q = fold(question);
    const CUES: [&str; 11] = [
        "primeira vez",
        "primeiro encontro",
        "primeira conversa",
        "primeira fala",
        "primeira interacao",
        "primeira coisa que",
        "se conheceram",
        "se conheciam",
        "quando conheceu",
        "quando conhece",
        "primeira aparicao",
    ];
    CUES.iter().any(|c| q.contains(c))
}

/// Earliest chunks (reading order) that mention the entities named in the
/// question. Chunks mentioning ALL of them come first; if there are too few, the
/// earliest chunks mentioning any single one fill the rest.
fn first_mention_chunks<'a>(
    question: &str,
    entity_names: &[String],
    chunks: &'a [Chunk],
    limit: usize,
) -> Vec<&'a Chunk> {
    let q = fold(question);
    // Seeds: known entity names that literally appear in the question.
    let mut seeds: Vec<String> = Vec::new();
    for name in entity_names {
        let f = fold(name);
        if f.chars().count() >= 3 && q.contains(&f) && !seeds.contains(&f) {
            seeds.push(f);
        }
    }
    if seeds.is_empty() {
        return Vec::new();
    }
    // Drop seeds that are contained in a longer seed ("Leo" inside "Leonardo") —
    // otherwise the short one matches chunks the full name never appears in.
    let all_seeds = seeds.clone();
    seeds.retain(|s| !all_seeds.iter().any(|o| o != s && o.contains(s.as_str())));

    let mut ordered: Vec<&Chunk> = chunks.iter().collect();
    ordered.sort_by(|a, b| cmp_doc(&a.doc_name, &b.doc_name).then(a.ordinal.cmp(&b.ordinal)));

    let mut out: Vec<&Chunk> = Vec::new();
    let mut partial: Vec<&Chunk> = Vec::new();
    for c in ordered {
        let t = fold(&c.text);
        let hits = seeds.iter().filter(|s| t.contains(s.as_str())).count();
        if hits == seeds.len() {
            if out.len() < limit {
                out.push(c);
            }
        } else if hits > 0 && partial.len() < limit {
            partial.push(c);
        }
        if out.len() >= limit {
            break;
        }
    }
    // Co-occurrence is the strong signal; single mentions only pad a thin result.
    if out.len() < 2 {
        for c in partial {
            if out.len() >= limit {
                break;
            }
            if !out.iter().any(|e| e.id == c.id) {
                out.push(c);
            }
        }
    }
    out
}

/// Strip a leading `<think>…</think>` reasoning block (closed or still open).
fn strip_think(raw: &str) -> String {
    let s = raw.trim_start();
    match s.find("<think>") {
        None => raw.to_string(),
        Some(open) => match s[open..].find("</think>") {
            Some(rel) => s[open + rel + "</think>".len()..].trim().to_string(),
            None => String::new(), // only reasoning arrived; no title yet
        },
    }
}

// ---- Entity extraction ----------------------------------------------------

#[derive(Debug, Deserialize)]
struct ExtractedChar {
    name: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    traits: Vec<String>,
    #[serde(default, alias = "sourceDoc")]
    source_doc: String,
    #[serde(default, alias = "sourceQuote")]
    source_quote: String,
}

#[derive(Debug, Deserialize)]
struct ExtractedPlace {
    name: String,
    #[serde(default, alias = "type")]
    kind: String,
    #[serde(default)]
    summary: String,
    #[serde(default, alias = "sourceDoc")]
    source_doc: String,
    #[serde(default, alias = "sourceQuote")]
    source_quote: String,
}

#[derive(Debug, Deserialize)]
struct ExtractedAbility {
    name: String,
    #[serde(default, alias = "type")]
    kind: String,
    #[serde(default)]
    summary: String,
    #[serde(default, alias = "sourceDoc")]
    source_doc: String,
    #[serde(default, alias = "sourceQuote")]
    source_quote: String,
}

#[derive(Debug, Deserialize)]
struct ExtractedRel {
    from: String,
    to: String,
    #[serde(default)]
    label: String,
}

#[derive(Debug, Deserialize)]
struct Extraction {
    #[serde(default)]
    characters: Vec<ExtractedChar>,
    #[serde(default)]
    places: Vec<ExtractedPlace>,
    #[serde(default)]
    abilities: Vec<ExtractedAbility>,
    #[serde(default)]
    relations: Vec<ExtractedRel>,
}

/// Result of an extraction run: the fresh entities/relations, plus the alias
/// maps (alias-lowercased → canonical display name) so the caller can fold
/// already-saved rows that turned out to be the same entity across runs.
pub struct ExtractResult {
    pub characters: Vec<Character>,
    pub places: Vec<Place>,
    pub abilities: Vec<Ability>,
    pub relations: Vec<Relation>,
    pub char_aliases: std::collections::HashMap<String, String>,
    pub place_aliases: std::collections::HashMap<String, String>,
}

/// Ask the LLM to read the vault's knowledge and extract characters, places and
/// relations as structured JSON.
pub async fn extract_entities(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
    existing_chars: &[String],
    existing_places: &[String],
    existing_abilities: &[String],
) -> AppResult<ExtractResult> {
    if chunks.is_empty() {
        return Err(AppError::Msg(
            "vault sem documentos indexados — carregue algo primeiro".into(),
        ));
    }

    // Extraction can use a dedicated (usually smaller) model. Empty = reuse the
    // chat model, so users without the VRAM for a second model pay no penalty.
    let ex_model = if cfg.extraction_model.trim().is_empty() {
        cfg.llm_model.as_str()
    } else {
        cfg.extraction_model.as_str()
    };

    // Split the whole corpus into ~12k-char windows on chunk boundaries, run the
    // LLM per window, and merge — so large works are covered, not just the head.
    const WINDOW_CHARS: usize = 12_000;
    // Cost/time guard. Raised again: 40 windows ≈ 480k chars, which a 29-chapter
    // work already overflows — the tail chapters were silently dropped, so their
    // characters simply never existed. Coverage beats speed here; incremental
    // extraction keeps normal runs (a chapter or two) small anyway, and only a full
    // re-extract pays the whole cost.
    const MAX_WINDOWS: usize = 150;
    let mut windows: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in chunks {
        cur.push_str(&format!("[{}]\n{}\n\n", c.doc_name, c.text));
        if cur.len() >= WINDOW_CHARS {
            windows.push(std::mem::take(&mut cur));
            if windows.len() >= MAX_WINDOWS {
                break;
            }
        }
    }
    if !cur.trim().is_empty() && windows.len() < MAX_WINDOWS {
        windows.push(cur);
    }

    // Accumulate + merge across windows.
    let mut chars_map: std::collections::HashMap<String, ExtractedChar> = Default::default();
    let mut places_map: std::collections::HashMap<String, ExtractedPlace> = Default::default();
    let mut abilities_map: std::collections::HashMap<String, ExtractedAbility> = Default::default();
    let mut rel_set: std::collections::HashSet<(String, String, String)> = Default::default();
    let mut relations_out: Vec<ExtractedRel> = Vec::new();

    // Every entity already saved in the vault is shown to the model, by name, in
    // every window. Without this the extractor re-invents a character it has
    // already met in an earlier chapter ("Leo", "Leonardo Venante") instead of
    // recognizing it and just adding what the new chapter reveals — merge_extracted
    // updates by name, so reusing the exact saved name is what turns a duplicate
    // into an update.
    let roster = roster_block(existing_chars, existing_places, existing_abilities);
    // Folded name → saved display name, so an emitted name that matches a saved
    // entity keeps the saved spelling instead of drifting.
    let known: std::collections::HashMap<String, String> = existing_chars
        .iter()
        .chain(existing_places.iter())
        .chain(existing_abilities.iter())
        .map(|n| (fold(n), n.clone()))
        .collect();

    // Run the windows with the configured concurrency. Default 1 (sequential) is
    // the safe choice for a single local GPU; cloud providers can raise it to cut
    // wall-clock time. A bad window is dropped (filter_map), not fatal.
    let concurrency = cfg.extraction_concurrency.max(1);
    let futs = windows
        .iter()
        .map(|w| extract_window(client, cfg, w, ex_model, &roster, &known))
        .collect::<Vec<_>>();
    let parsed_windows: Vec<Extraction> = stream::iter(futs)
        .buffer_unordered(concurrency)
        .filter_map(|r| async move { r.ok() })
        .collect()
        .await;

    for parsed in parsed_windows {
        for c in parsed.characters {
            if c.name.trim().is_empty() {
                continue;
            }
            merge_char(&mut chars_map, c);
        }
        for p in parsed.places {
            if p.name.trim().is_empty() {
                continue;
            }
            merge_place(&mut places_map, p);
        }
        for a in parsed.abilities {
            if a.name.trim().is_empty() {
                continue;
            }
            merge_ability(&mut abilities_map, a);
        }
        for r in parsed.relations {
            if r.from.trim().is_empty() || r.to.trim().is_empty() {
                continue;
            }
            let key = (r.from.to_lowercase(), r.to.to_lowercase(), r.label.to_lowercase());
            if rel_set.insert(key) {
                relations_out.push(r);
            }
        }
    }

    if chars_map.is_empty() && places_map.is_empty() && abilities_map.is_empty() {
        return Err(AppError::Msg(
            "o modelo não retornou entidades válidas — tente outro modelo de LLM".into(),
        ));
    }

    // Coreference: "Cesar" and "Cesar Magnus" are the same character. Canonicalize
    // each partial name to the fullest matching name (within its own type), then
    // re-merge and rewrite relation endpoints so the graph links them as one.
    // Include already-saved entity names so a name introduced in THIS run
    // ("Cesar") resolves to a fuller name saved in a PREVIOUS run ("Cesar Magnus"),
    // fixing the cross-run split. The canonical target may be an existing entity;
    // merge_extracted then folds the new data into that saved card by name.
    let mut char_names: Vec<String> = chars_map.values().map(|c| c.name.clone()).collect();
    let mut place_names: Vec<String> = places_map.values().map(|p| p.name.clone()).collect();
    for n in existing_chars {
        if !char_names.iter().any(|e| e.eq_ignore_ascii_case(n)) {
            char_names.push(n.clone());
        }
    }
    for n in existing_places {
        if !place_names.iter().any(|e| e.eq_ignore_ascii_case(n)) {
            place_names.push(n.clone());
        }
    }
    // Abilities get the same treatment (in-run only — no cross-run alias rewrite is
    // needed, since folding onto the saved name is what merge_extracted keys on).
    let mut ability_names: Vec<String> = abilities_map.values().map(|a| a.name.clone()).collect();
    for n in existing_abilities {
        if !ability_names.iter().any(|e| e.eq_ignore_ascii_case(n)) {
            ability_names.push(n.clone());
        }
    }
    let ability_canon = canonical_map(&ability_names);

    let mut char_canon = canonical_map(&char_names);
    let mut place_canon = canonical_map(&place_names);

    // Optional LLM dedup: catches aliases the substring heuristic can't (titles,
    // nicknames like "o Caçador" = "Cesar Magnus"). Targeted — only names that
    // share a token with another are sent (the dubious ones); unambiguous names
    // are skipped, keeping the payload small. Failures are non-fatal.
    if cfg.dedup_entities {
        let char_dubious = dubious_candidates(&char_names);
        let place_dubious = dubious_candidates(&place_names);
        if let Ok(m) = llm_dedup(client, cfg, "personagens", &char_dubious, ex_model).await {
            for (k, v) in m { char_canon.insert(k, v); }
            chain_resolve(&mut char_canon);
        }
        if let Ok(m) = llm_dedup(client, cfg, "lugares", &place_dubious, ex_model).await {
            for (k, v) in m { place_canon.insert(k, v); }
            chain_resolve(&mut place_canon);
        }
    }

    let mut chars_final: std::collections::HashMap<String, ExtractedChar> = Default::default();
    for mut c in chars_map.into_values() {
        if let Some(full) = char_canon.get(&c.name.to_lowercase()) {
            c.name = full.clone();
        }
        merge_char(&mut chars_final, c);
    }
    let mut places_final: std::collections::HashMap<String, ExtractedPlace> = Default::default();
    for mut p in places_map.into_values() {
        if let Some(full) = place_canon.get(&p.name.to_lowercase()) {
            p.name = full.clone();
        }
        merge_place(&mut places_final, p);
    }
    let mut abilities_final: std::collections::HashMap<String, ExtractedAbility> = Default::default();
    for mut a in abilities_map.into_values() {
        if let Some(full) = ability_canon.get(&a.name.to_lowercase()) {
            a.name = full.clone();
        }
        merge_ability(&mut abilities_final, a);
    }
    let chars_map = chars_final;
    let places_map = places_final;
    let abilities_map = abilities_final;

    // Rewrite relations to canonical names (either type) and de-dupe again.
    let resolve = |name: &str| -> String {
        let k = name.to_lowercase();
        char_canon
            .get(&k)
            .or_else(|| place_canon.get(&k))
            .cloned()
            .unwrap_or_else(|| name.to_string())
    };
    let mut seen: std::collections::HashSet<(String, String, String)> = Default::default();
    let relations_out: Vec<ExtractedRel> = relations_out
        .into_iter()
        .map(|r| ExtractedRel { from: resolve(&r.from), to: resolve(&r.to), label: r.label })
        .filter(|r| {
            if r.from.eq_ignore_ascii_case(&r.to) {
                return false; // self-loop after canonicalization
            }
            seen.insert((r.from.to_lowercase(), r.to.to_lowercase(), r.label.to_lowercase()))
        })
        .collect();

    let characters = chars_map
        .into_values()
        .map(|c| {
            // Keep traits as a short set of ONE-WORD tags (≤ 6). Anything longer is
            // really a description, not a tag — fold it into the summary so the card
            // stays scannable instead of a wall of phrases.
            let (traits, summary) = normalize_traits(c.traits, c.summary);
            Character {
                id: Uuid::new_v4().to_string(),
                name: c.name,
                role: c.role,
                summary,
                traits,
                status: "Extraído".into(),
                source_doc: c.source_doc,
                source_quote: c.source_quote,
            }
        })
        .collect();

    let abilities = abilities_map
        .into_values()
        .map(|a| Ability {
            id: Uuid::new_v4().to_string(),
            name: a.name,
            kind: if a.kind.trim().is_empty() { "Poder".into() } else { a.kind },
            summary: a.summary,
            status: "Extraído".into(),
            source_doc: a.source_doc,
            source_quote: a.source_quote,
        })
        .collect();

    let places = places_map
        .into_values()
        .map(|p| Place {
            id: Uuid::new_v4().to_string(),
            name: p.name,
            kind: if p.kind.is_empty() { "Local".into() } else { p.kind },
            summary: p.summary,
            status: "Extraído".into(),
            source_doc: p.source_doc,
            source_quote: p.source_quote,
        })
        .collect();

    let relations = relations_out
        .into_iter()
        .map(|r| Relation { from: r.from, to: r.to, label: r.label })
        .collect();

    // Keep only aliases that actually point elsewhere — the caller uses these to
    // merge previously-saved rows into their canonical name.
    char_canon.retain(|k, v| *k != v.to_lowercase());
    place_canon.retain(|k, v| *k != v.to_lowercase());

    Ok(ExtractResult {
        characters,
        places,
        abilities,
        relations,
        char_aliases: char_canon,
        place_aliases: place_canon,
    })
}

/// Split raw extracted traits into (single-word tags, extra prose). Keeps up to 6
/// one-word tags; any multi-word "trait" is really a description and is appended
/// to the summary instead. De-dupes tags case-insensitively.
fn normalize_traits(traits: Vec<String>, summary: String) -> (Vec<String>, String) {
    const MAX_TRAITS: usize = 6;
    let mut tags: Vec<String> = Vec::new();
    let mut extra: Vec<String> = Vec::new();
    for t in traits {
        let t = t.trim().trim_matches(|c: char| c == '.' || c == ',' || c == ';').trim();
        if t.is_empty() {
            continue;
        }
        // One "word": no internal whitespace and no hyphen-joined compound.
        let is_single = !t.chars().any(|c| c.is_whitespace()) && !t.contains('-');
        if is_single {
            if tags.len() < MAX_TRAITS && !tags.iter().any(|e| e.eq_ignore_ascii_case(t)) {
                tags.push(t.to_string());
            }
        } else {
            extra.push(t.to_string());
        }
    }
    let summary = if extra.is_empty() {
        summary
    } else {
        let joined = extra.join("; ");
        if summary.trim().is_empty() {
            joined
        } else {
            format!("{} {}.", summary.trim_end_matches('.'), joined)
        }
    };
    (tags, summary)
}

/// Merge an ability into the map: first non-empty text wins, longest summary kept.
fn merge_ability(map: &mut std::collections::HashMap<String, ExtractedAbility>, a: ExtractedAbility) {
    let key = a.name.to_lowercase();
    match map.get_mut(&key) {
        None => {
            map.insert(key, a);
        }
        Some(existing) => {
            if existing.kind.is_empty() { existing.kind = a.kind; }
            if existing.summary.len() < a.summary.len() { existing.summary = a.summary; }
            if existing.source_doc.is_empty() { existing.source_doc = a.source_doc; }
            if existing.source_quote.is_empty() { existing.source_quote = a.source_quote; }
        }
    }
}

/// Run extraction over a single corpus window.
/// The "already known" section of the extraction prompt: every saved entity, by
/// name and by type. Capped only to keep a huge cast from crowding out the text
/// itself — the whole roster is sent whenever it fits.
fn roster_block(chars: &[String], places: &[String], abilities: &[String]) -> String {
    if chars.is_empty() && places.is_empty() && abilities.is_empty() {
        return String::new();
    }
    // Roughly a third of the window budget; beyond that the corpus would lose room.
    const MAX_ROSTER: usize = 12_000;
    let mut out = String::from(
        "ENTIDADES JÁ CADASTRADAS NESTA OBRA (lidas de capítulos anteriores). \
Se uma delas aparecer no texto abaixo, use EXATAMENTE o nome desta lista, copiado letra \
por letra, e apenas ACRESCENTE o que o novo texto revela — NÃO crie uma segunda entrada, \
NÃO troque por apelido, epíteto ou título, NÃO acrescente sobrenome. Só crie entidade \
nova se ela realmente não estiver na lista.\n",
    );
    let mut push = |label: &str, names: &[String]| {
        if names.is_empty() {
            return;
        }
        let mut line = format!("{label}: ");
        let mut n = 0usize;
        for name in names {
            if out.len() + line.len() > MAX_ROSTER {
                line.push_str(&format!("… (+{} não listados)", names.len() - n));
                break;
            }
            if n > 0 {
                line.push_str("; ");
            }
            line.push_str(name);
            n += 1;
        }
        out.push_str(&line);
        out.push('\n');
    };
    push("Personagens", chars);
    push("Lugares", places);
    push("Habilidades", abilities);
    out
}

async fn extract_window(
    client: &reqwest::Client,
    cfg: &RagConfig,
    corpus: &str,
    model: &str,
    roster: &str,
    known: &std::collections::HashMap<String, String>,
) -> AppResult<Extraction> {
    let rules = "Você extrai entidades de textos de ficção/worldbuilding. \
Responda APENAS com JSON válido, sem texto extra, sem markdown. Formato:\n\
{\"characters\":[{\"name\":\"\",\"role\":\"\",\"summary\":\"\",\"traits\":[\"\"],\"sourceDoc\":\"\",\"sourceQuote\":\"\"}],\
\"places\":[{\"name\":\"\",\"type\":\"\",\"summary\":\"\",\"sourceDoc\":\"\",\"sourceQuote\":\"\"}],\
\"abilities\":[{\"name\":\"\",\"type\":\"\",\"summary\":\"\",\"sourceDoc\":\"\",\"sourceQuote\":\"\"}],\
\"relations\":[{\"from\":\"\",\"to\":\"\",\"label\":\"\"}]}\n\
Seja CONCISO para responder rápido: \"summary\" com 1 a 2 frases curtas; \"sourceQuote\" \
uma citação literal de no máximo ~100 caracteres. Não repita informação nem escreva prosa fora do JSON. \
Use o idioma do texto. \
REGRA DE NOME (crítica): copie o nome EXATAMENTE como ele aparece escrito no texto. \
NUNCA invente sobrenome nem junte palavras próximas para formar um nome — títulos, classes, \
espécies, facções, grupos e tipos de poder NÃO são sobrenomes. Se o texto só usa o primeiro nome \
ou um apelido, use exatamente ele. Um nome por entrada: nada de \"Leo / Leonardo\", \
\"Leonardo (Leo)\" ou variações no mesmo campo \"name\". Nas relações, use exatamente esses \
mesmos nomes.\n\
Extraia TODOS os personagens — SERES (pessoas, criaturas, entidades vivas ou sencientes) — \
INCLUSIVE os citados por apelido, epíteto, título ou cargo (ex.: \"Salazar Bessa\", \"David Bessa\", \
\"A Bruxa\", \"Rei Yan Serafine\", \"o Caçador\"): todos são personagens. NUNCA deixe de fora um ser só \
porque ele é chamado por um título ou epíteto. Não seja conservador com personagens — na dúvida se um \
NOME próprio se refere a um ser, inclua como personagem.\n\
\"abilities\" (à parte, opcional) é SOMENTE para poderes/magias/técnicas que um personagem possui ou usa \
(ex.: \"Previsão\", \"Hipótese\", \"Teletransporte\", ou \"Espadas do Julgamento\" QUANDO o texto a trata como \
poder/técnica conjurado). \"type\" = categoria (\"Poder\", \"Magia\", \"Técnica\"). Decida pelo CONTEXTO.\n\
NÃO são habilidades (ignore, a menos que sejam personagens/lugares): objetos, armas ou artefatos, \
mesmo sagrados ou nomeados (ex.: \"uma espada sagrada\", \"uma espada de brilho roxo\"); \
organizações, facções, ordens ou eventos históricos (ex.: \"a Inquisição\" = Santa Inquisição). \
Contexto: \"Espadas do Julgamento\" que alguém CONJURA = ability; \"uma espada sagrada\" empunhada = não.\n\
Na dúvida sobre HABILIDADE, não inclua; é NORMAL capítulos iniciais não terem nenhuma — deixe \"abilities\" \
vazio se não houver poder claro. Essa cautela vale só para abilities, JAMAIS para personagens.\n\
Em \"traits\", liste NO MÁXIMO 6 tags de UMA ÚNICA palavra cada (ex.: \"Leal\", \"Impulsivo\"). \
NÃO use frases nem expressões de várias palavras em traits — qualquer descrição mais longa vai no \"summary\".";

    let system = if roster.is_empty() {
        format!("{rules} /no_think")
    } else {
        format!("{rules}\n\n{roster}\n/no_think")
    };

    let user = format!("Extraia personagens, lugares e relações do conhecimento abaixo:\n\n{corpus}");
    let messages = vec![
        ChatMessage { role: "system", content: system },
        ChatMessage { role: "user", content: user },
    ];
    let raw = providers::chat_internal(client, cfg, &messages, model).await?;
    // Reasoning models (and Ollama's `thinking` field) prepend a <think>…</think>
    // block whose braces would corrupt the JSON span — drop it before parsing.
    let raw = strip_think(&raw);
    let json = extract_json_block(&raw)
        .ok_or_else(|| AppError::Msg("modelo não retornou JSON válido para extração".into()))?;
    let parsed: Extraction = serde_json::from_str(&json)
        .map_err(|e| AppError::Msg(format!("falha ao ler JSON da extração: {e}")))?;
    Ok(ground_extraction(parsed, corpus, known))
}

/// Keep only what the source text actually says.
///
/// Every entity name is rewritten to the longest run of its own words that
/// appears VERBATIM in the window the model just read; entities with no grounded
/// form at all are dropped. This is what kills the two worst failure modes at
/// scale: outright invented characters/abilities, and fabricated surnames —
/// "Leonardo Venante" (where "Venante" is the name of the powered class, not a
/// family name) has no verbatim occurrence, so it collapses back to "Leonardo",
/// which also folds the "Leo / Leonardo" and "Leonardo (Leo) Venante" duplicates
/// into the same entity instead of three dead vertices.
/// `known` maps folded → saved display name of entities already in the vault. A
/// name that matches one of them keeps the SAVED spelling (as long as the text
/// really mentions it), so a character met in chapter 3 is recognized in chapter
/// 20 and updated instead of duplicated.
fn ground_extraction(
    mut ex: Extraction,
    corpus: &str,
    known: &std::collections::HashMap<String, String>,
) -> Extraction {
    let hay = fold(corpus);
    let ground_name = |raw: &str| ground_known_name(raw, &hay, known);

    ex.characters.retain_mut(|c| match ground_name(&c.name) {
        Some(n) => {
            c.name = n;
            c.source_quote = grounded_quote(&c.source_quote, &hay);
            true
        }
        None => false,
    });
    ex.places.retain_mut(|p| match ground_name(&p.name) {
        Some(n) => {
            p.name = n;
            p.source_quote = grounded_quote(&p.source_quote, &hay);
            true
        }
        None => false,
    });
    ex.abilities.retain_mut(|a| match ground_name(&a.name) {
        Some(n) => {
            a.name = n;
            a.source_quote = grounded_quote(&a.source_quote, &hay);
            true
        }
        None => false,
    });
    // Relation endpoints get the same treatment, so edges point at the grounded
    // entity names rather than at hallucinated variants (which render as extra,
    // unusable vertices in the graph).
    ex.relations.retain_mut(|r| {
        match (ground_name(&r.from), ground_name(&r.to)) {
            (Some(f), Some(t)) if !f.eq_ignore_ascii_case(&t) => {
                r.from = f;
                r.to = t;
                true
            }
            _ => false,
        }
    });
    ex
}

/// Normalize a model-emitted entity name: drop parenthetical alias blocks, keep
/// the longest alternative of a "A / B" form, keep only the part before an
/// em-dash epithet, and collapse whitespace/edge punctuation.
fn clean_name(raw: &str) -> String {
    // Remove "(...)" / "[...]" alias blocks: "Leonardo (Leo) Venante" → "Leonardo Venante".
    let mut s = String::with_capacity(raw.len());
    let mut depth = 0i32;
    for ch in raw.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            _ if depth == 0 => s.push(ch),
            _ => {}
        }
    }
    // "Leonardo — o Caçador": the name precedes the epithet.
    for sep in [" — ", " – ", " - "] {
        if let Some(i) = s.find(sep) {
            s.truncate(i);
        }
    }
    // "Leo / Leonardo": alternative spellings of one name — keep the fullest.
    if s.contains('/') || s.contains('|') {
        s = s
            .split(|c| c == '/' || c == '|')
            .max_by_key(|p| p.trim().chars().count())
            .unwrap_or(&s)
            .to_string();
    }
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == '.' || c == ':' || c == ';')
        .trim()
        .to_string()
}

/// Occurrences of `needle` in `hay` that stand on word boundaries. Plain
/// substring matching is not enough: "do" would "ground" itself inside
/// "Leonardo", letting a hallucinated "Chamas do Vazio" survive as the entity
/// "do".
fn word_occurrences(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let bytes = hay.as_bytes();
    let boundary = |i: usize| -> bool {
        if i == 0 || i >= bytes.len() {
            return true;
        }
        !hay[..i]
            .chars()
            .next_back()
            .map_or(false, |c| c.is_alphanumeric())
    };
    let mut count = 0;
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let ends_clean = hay[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        if boundary(start) && ends_clean {
            count += 1;
        }
        from = start + needle.len().max(1);
        if from >= hay.len() {
            break;
        }
    }
    count
}

/// Function words that must never survive as an entity name on their own — they
/// appear everywhere, so a hallucinated multi-word name could otherwise ground
/// itself down to one of them.
const NAME_STOPWORDS: &[&str] = &[
    "a", "o", "as", "os", "um", "uma", "uns", "umas", "de", "do", "da", "dos", "das", "em", "no",
    "na", "nos", "nas", "ao", "aos", "e", "ou", "que", "com", "por", "para", "sem", "seu", "sua",
    "ele", "ela", "eles", "elas", "isso", "esse", "essa", "este", "esta", "the", "of",
];

/// Ground a name, preferring the spelling already saved in the vault.
///
/// If the emitted name matches a saved entity AND the text gives evidence of the
/// mention, the SAVED name is returned — so "Leonardo Silva" reported over a
/// chapter that only writes "Leonardo" updates the existing card rather than
/// spawning a second one. Evidence is still required: a saved name the window
/// never mentions is dropped, so the roster can't be copied in wholesale.
fn ground_known_name(
    raw: &str,
    hay: &str,
    known: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let corpus_form = ground_name(raw, hay)?;
    let cleaned = clean_name(raw);
    Some(
        known
            .get(&fold(&cleaned))
            .or_else(|| known.get(&fold(&corpus_form)))
            .cloned()
            .unwrap_or(corpus_form),
    )
}

/// The longest run of consecutive words of `raw` that appears verbatim in the
/// folded corpus `hay`. Ties break on how often the run occurs (a real name is
/// repeated; an accidental fragment is not), then leftmost. `None` = the model
/// made the name up.
fn ground_name(raw: &str, hay: &str) -> Option<String> {
    let name = clean_name(raw);
    if name.is_empty() {
        return None;
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    // A "name" longer than this is a sentence, not an entity.
    if words.is_empty() || words.len() > 8 {
        return None;
    }
    for len in (1..=words.len()).rev() {
        let mut best: Option<(usize, usize, String)> = None; // (count, -start, text)
        for start in 0..=(words.len() - len) {
            let run = words[start..start + len].join(" ");
            let folded = fold(&run);
            if folded.chars().count() < 2 {
                continue;
            }
            // A lone function word is never a name, however often it occurs.
            if len == 1 && NAME_STOPWORDS.contains(&folded.as_str()) {
                continue;
            }
            let count = word_occurrences(hay, &folded);
            if count == 0 {
                continue;
            }
            let better = match &best {
                None => true,
                Some((bc, bs, _)) => count > *bc || (count == *bc && start < *bs),
            };
            if better {
                best = Some((count, start, run));
            }
        }
        if let Some((_, _, run)) = best {
            return Some(run);
        }
    }
    None
}

/// Keep `quote` only if it really occurs in the source; otherwise blank it out —
/// a fabricated "literal quote" is worse than none, since the UI presents it as
/// evidence.
fn grounded_quote(quote: &str, hay: &str) -> String {
    let q = quote.trim().trim_matches('"').trim();
    if q.chars().count() < 8 {
        return String::new();
    }
    if hay.contains(fold(q).as_str()) {
        q.to_string()
    } else {
        String::new()
    }
}

/// Merge a character into the map: first non-empty text wins, traits union.
fn merge_char(map: &mut std::collections::HashMap<String, ExtractedChar>, c: ExtractedChar) {
    let key = c.name.to_lowercase();
    match map.get_mut(&key) {
        None => {
            map.insert(key, c);
        }
        Some(existing) => {
            if existing.role.is_empty() { existing.role = c.role; }
            if existing.summary.len() < c.summary.len() { existing.summary = c.summary; }
            if existing.source_doc.is_empty() { existing.source_doc = c.source_doc; }
            if existing.source_quote.is_empty() { existing.source_quote = c.source_quote; }
            for t in c.traits {
                if !existing.traits.iter().any(|e| e.eq_ignore_ascii_case(&t)) {
                    existing.traits.push(t);
                }
            }
        }
    }
}

fn merge_place(map: &mut std::collections::HashMap<String, ExtractedPlace>, p: ExtractedPlace) {
    let key = p.name.to_lowercase();
    match map.get_mut(&key) {
        None => {
            map.insert(key, p);
        }
        Some(existing) => {
            if existing.kind.is_empty() { existing.kind = p.kind; }
            if existing.summary.len() < p.summary.len() { existing.summary = p.summary; }
            if existing.source_doc.is_empty() { existing.source_doc = p.source_doc; }
            if existing.source_quote.is_empty() { existing.source_quote = p.source_quote; }
        }
    }
}

/// Folded word tokens (len ≥ 2) of a name.
fn name_tokens(name: &str) -> Vec<String> {
    fold(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 2)
        .map(|t| t.to_string())
        .collect()
}

/// Titles/honorifics and articles that precede a name. Stripped before comparing
/// so "Rei Yan Serafine" and "Yan Serafine" are recognized as one character.
const HONORIFICS: &[&str] = &[
    "rei", "rainha", "principe", "princesa", "lorde", "lady", "dom", "dona", "sir", "senhor",
    "senhora", "padre", "frei", "irmao", "irma", "mestre", "mestra", "capitao", "comandante",
    "general", "doutor", "doutora", "professor", "professora", "sao", "santa", "santo", "conde",
    "condessa", "duque", "duquesa", "barao", "cavaleiro", "tio", "tia", "the", "los",
];

/// Name tokens with leading honorifics/articles removed — the identity-bearing
/// part of the name.
fn core_tokens(name: &str) -> Vec<String> {
    let mut t = name_tokens(name);
    while t.len() > 1 && HONORIFICS.contains(&t[0].as_str()) {
        t.remove(0);
    }
    t
}

/// Whether every token of `short` also appears in `long` (order-insensitive).
/// Order-insensitive on purpose: the model emits "Leo / Leonardo" → tokens
/// [leo, leonardo], which is NOT an in-order subsequence of "Leonardo (Leo)
/// Venante" → [leonardo, leo, venante], so the old ordered check missed it.
fn is_subset(short: &[String], long: &[String]) -> bool {
    short.iter().all(|s| long.contains(s))
}

/// Is `short` a plausible diminutive/short form of `long` ("leo" → "leonardo")?
/// Requires a real prefix of at least 3 characters, so "ana" ↛ "andre".
fn is_diminutive(short: &str, long: &str) -> bool {
    short.chars().count() >= 3 && long.chars().count() > short.chars().count() && long.starts_with(short)
}

/// Names worth sending to the LLM dedup pass: those sharing at least one token
/// with another name (the ambiguous ones — "Cesar" ↔ "Cesar Magnus", "Rei
/// Aldric" ↔ "Aldric"). Names with no token overlap can't be aliases of anything
/// here, so they're skipped to keep the dedup payload small and targeted.
fn dubious_candidates(names: &[String]) -> Vec<String> {
    let toks: Vec<Vec<String>> = names.iter().map(|n| core_tokens(n)).collect();
    let mut out = Vec::new();
    for (i, ti) in toks.iter().enumerate() {
        if ti.is_empty() {
            continue;
        }
        // Shares a whole token with another name ("Cesar" ↔ "Cesar Magnus")...
        let shares = toks
            .iter()
            .enumerate()
            .any(|(j, tj)| j != i && tj.iter().any(|t| ti.contains(t)));
        // ...or is a one-word short form whose prefix matches another name
        // ("Leo" ↔ "Leonardo" — no shared token, so the old gate excluded it and
        // the nickname never even reached the dedup pass)...
        let prefixes = ti.len() == 1
            && toks.iter().enumerate().any(|(j, tj)| {
                j != i && tj.iter().any(|t| is_diminutive(&ti[0], t) || is_diminutive(t, &ti[0]))
            });
        // ...or is a short one-word name, the shape nicknames/epithets take
        // ("Lô" for Charlotte), which only the LLM can resolve.
        let nickname_shape = ti.len() == 1 && ti[0].chars().count() <= 5;
        if shares || prefixes || nickname_shape {
            out.push(names[i].clone());
        }
    }
    out
}

/// Map each partial name (folded) to its canonical display name:
///   * honorific variants collapse onto the untitled form ("Rei Yan Serafine" → "Yan Serafine");
///   * a name whose words are all contained in a longer name folds into it ("Cesar" → "Cesar Magnus");
///   * an unambiguous one-word short form folds into its long form ("Leo" → "Leonardo").
/// Resolution is chained so aliases collapse to a single canonical name.
fn canonical_map(names: &[String]) -> std::collections::HashMap<String, String> {
    let info: Vec<(String, Vec<String>)> =
        names.iter().map(|n| (n.clone(), core_tokens(n))).collect();
    let mut out: std::collections::HashMap<String, String> = Default::default();

    // Pass 1 — same identity core, different honorifics: prefer the plain name.
    for (name, t) in &info {
        if t.is_empty() || name_tokens(name).len() == t.len() {
            continue; // no honorific to drop
        }
        if let Some(plain) = info
            .iter()
            .find(|(o, ot)| ot == t && name_tokens(o).len() == ot.len())
        {
            // Keys stay plain-lowercased (not accent-folded): db::apply_aliases
            // matches them against SQL lower(name).
            out.insert(name.to_lowercase(), plain.0.clone());
        }
    }

    // Pass 2 — containment and unambiguous short forms.
    for (name, t) in &info {
        if t.is_empty() || out.contains_key(&name.to_lowercase()) {
            continue;
        }
        let mut best: Option<&(String, Vec<String>)> = None;
        let mut dim: Option<&(String, Vec<String>)> = None;
        let mut dim_count = 0usize;
        for cand in &info {
            if cand.0.eq_ignore_ascii_case(name) || cand.1.is_empty() {
                continue;
            }
            if cand.1.len() > t.len() && is_subset(t, &cand.1) {
                if best.map_or(true, |b| b.1.len() < cand.1.len()) {
                    best = Some(cand);
                }
            } else if t.len() == 1 && is_diminutive(&t[0], &cand.1[0]) {
                // Count every long form this short name could belong to; fold only
                // when exactly one exists, so "Leo" never guesses between
                // "Leonardo" and "Leopoldo".
                dim_count += 1;
                dim = Some(cand);
            }
        }
        let pick = best.or(if dim_count == 1 { dim } else { None });
        if let Some(b) = pick {
            out.insert(name.to_lowercase(), b.0.clone());
        }
    }

    chain_resolve(&mut out);
    out
}

/// Collapse alias chains (a → b → c) so every key points at the fullest name.
fn chain_resolve(out: &mut std::collections::HashMap<String, String>) {
    let keys: Vec<String> = out.keys().cloned().collect();
    for k in keys {
        let mut cur = out.get(&k).cloned().unwrap();
        let mut guard = 0;
        while let Some(next) = out.get(&cur.to_lowercase()) {
            if *next == cur || guard > 8 {
                break;
            }
            cur = next.clone();
            guard += 1;
        }
        out.insert(k, cur);
    }
}

#[derive(Debug, Deserialize)]
struct DedupGroup {
    #[serde(default)]
    canonical: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DedupResult {
    #[serde(default)]
    groups: Vec<DedupGroup>,
}

/// Ask the LLM which names in `names` refer to the same entity. Returns a map of
/// alias (lowercased) → canonical display name. Only groups of 2+ matter.
async fn llm_dedup(
    client: &reqwest::Client,
    cfg: &RagConfig,
    kind: &str,
    names: &[String],
    model: &str,
) -> AppResult<std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::new();
    if names.len() < 2 {
        return Ok(out);
    }
    // One giant list degrades badly: a 29-chapter work yields hundreds of names and
    // the model starts truncating and inventing. Batch it, and let a failed batch
    // cost only that batch.
    const BATCH: usize = 60;
    for batch in names.chunks(BATCH) {
        if batch.len() < 2 {
            continue;
        }
        if let Ok(m) = llm_dedup_batch(client, cfg, kind, batch, model).await {
            out.extend(m);
        }
    }
    Ok(out)
}

async fn llm_dedup_batch(
    client: &reqwest::Client,
    cfg: &RagConfig,
    kind: &str,
    names: &[String],
    model: &str,
) -> AppResult<std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::new();
    let list = names
        .iter()
        .map(|n| format!("- {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let system = format!(
        "Você recebe uma lista de nomes de {kind} extraídos da MESMA obra. \
Alguns são formas diferentes do MESMO ser: primeiro nome, apelido, diminutivo (\"Leo\" para \
\"Leonardo\"), epíteto, título, ou a mesma pessoa com e sem sobrenome. Agrupe essas formas. \
Responda APENAS com JSON, sem markdown:\n\
{{\"groups\":[{{\"canonical\":\"nome da lista\",\"aliases\":[\"outro nome da lista\",\"...\"]}}]}}\n\
REGRAS: \"canonical\" e cada alias DEVEM ser copiados EXATAMENTE de um nome da lista — \
nunca escreva um nome que não está na lista e nunca combine dois nomes num só. \
Escolha como \"canonical\" a forma mais usada e mais simples (sem título). \
NÃO agrupe seres diferentes — parentes que compartilham sobrenome (pai e filho, irmãos) \
são pessoas DISTINTAS. Inclua SOMENTE grupos com 2 ou mais nomes; sem duplicatas, \
retorne {{\"groups\":[]}}. /no_think"
    );
    let user = format!("Nomes:\n{list}");
    let messages = vec![
        ChatMessage { role: "system", content: system },
        ChatMessage { role: "user", content: user },
    ];
    let raw = providers::chat_internal(client, cfg, &messages, model).await?;
    let raw = strip_think(&raw);
    let json = extract_json_block(&raw)
        .ok_or_else(|| AppError::Msg("dedup: JSON inválido".into()))?;
    let parsed: DedupResult =
        serde_json::from_str(&json).map_err(|e| AppError::Msg(format!("dedup: {e}")))?;

    // Only trust names that actually came from the list we sent.
    let known: std::collections::HashMap<String, String> =
        names.iter().map(|n| (n.to_lowercase(), n.clone())).collect();
    for g in parsed.groups {
        // The canonical MUST be one of the given names. Without this check the model
        // is free to invent a display name for the whole group (e.g. gluing a class
        // name on as a surname), and every alias then points at a name that exists
        // nowhere in the text.
        let members: Vec<String> = g
            .aliases
            .iter()
            .chain(std::iter::once(&g.canonical))
            .filter_map(|n| known.get(&n.to_lowercase()).cloned())
            .collect();
        if members.len() < 2 {
            continue;
        }
        let canonical = match known.get(&g.canonical.to_lowercase()) {
            Some(c) => c.clone(),
            // Invented canonical → fall back to the group's shortest real name,
            // which is the plainest form (titles and epithets only add words).
            None => members
                .iter()
                .min_by_key(|n| (name_tokens(n).len(), n.chars().count()))
                .cloned()
                .unwrap_or_default(),
        };
        if canonical.is_empty() {
            continue;
        }
        for alias in members {
            let low = alias.to_lowercase();
            if !low.eq_ignore_ascii_case(&canonical) {
                out.insert(low, canonical.clone());
            }
        }
    }
    Ok(out)
}

/// Pull the outermost `{ ... }` JSON object out of a possibly noisy LLM reply
/// (handles ```json fences and stray prose).
fn extract_json_block(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end > start {
        Some(raw[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real-world corpus shape: the text says "Leonardo" and separately talks
    /// about "Venante" as the name of the powered class — never "Leonardo Venante".
    const CORPUS: &str = "Leonardo olhou para Charlotte. Os Venante nasciam com poderes. \
Leonardo sorriu. Charlotte respondeu: — Você é um deles? Salazar Bessa observava. \
O Rei Yan Serafine chegou depois. Leo correu.";

    #[test]
    fn fabricated_surname_collapses_to_the_grounded_name() {
        let hay = fold(CORPUS);
        // All three model variants ground to the same real name → one entity, not three.
        assert_eq!(ground_name("Leonardo Venante", &hay).as_deref(), Some("Leonardo"));
        assert_eq!(ground_name("Leonardo (Leo) Venante", &hay).as_deref(), Some("Leonardo"));
        assert_eq!(ground_name("Leo / Leonardo", &hay).as_deref(), Some("Leonardo"));
        // A name the text never contains is a hallucination and is dropped.
        assert_eq!(ground_name("Mirela Aldaravi", &hay), None);
        // A name written verbatim survives intact.
        assert_eq!(ground_name("Salazar Bessa", &hay).as_deref(), Some("Salazar Bessa"));
    }

    #[test]
    fn hallucinated_entities_are_filtered_out() {
        let ex = Extraction {
            characters: vec![
                ExtractedChar { name: "Leonardo Venante".into(), role: String::new(), summary: String::new(), traits: vec![], source_doc: String::new(), source_quote: "frase que nunca foi escrita".into() },
                ExtractedChar { name: "Personagem Inexistente".into(), role: String::new(), summary: String::new(), traits: vec![], source_doc: String::new(), source_quote: String::new() },
            ],
            places: vec![],
            abilities: vec![ExtractedAbility { name: "Chamas do Vazio".into(), kind: String::new(), summary: String::new(), source_doc: String::new(), source_quote: String::new() }],
            relations: vec![ExtractedRel { from: "Leonardo Venante".into(), to: "Charlotte".into(), label: "conhece".into() }],
        };
        let g = ground_extraction(ex, CORPUS, &Default::default());
        assert_eq!(g.characters.len(), 1);
        assert_eq!(g.characters[0].name, "Leonardo");
        assert_eq!(g.characters[0].source_quote, "", "citação inventada deve ser apagada");
        assert!(g.abilities.is_empty(), "habilidade inventada deve sair");
        // The relation endpoint was rewritten to the grounded name.
        assert_eq!(g.relations.len(), 1);
        assert_eq!(g.relations[0].from, "Leonardo");
    }

    /// The whole point of showing the saved cast to the extractor: a character met
    /// in an earlier chapter must be recognized and updated, never duplicated.
    #[test]
    fn saved_entities_are_reused_not_duplicated() {
        let hay = fold(CORPUS);
        let known: std::collections::HashMap<String, String> =
            [("charlotte", "Charlotte Bessa"), ("leonardo", "Leonardo")]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
        // The chapter writes only "Charlotte"; the saved card is "Charlotte Bessa".
        // Either spelling must land on the saved name so merge_extracted updates it.
        assert_eq!(ground_known_name("Charlotte Bessa", &hay, &known).as_deref(), Some("Charlotte Bessa"));
        assert_eq!(ground_known_name("Charlotte", &hay, &known).as_deref(), Some("Charlotte Bessa"));
        // A saved name the window never mentions gets no free pass — the roster
        // must not be copied in wholesale.
        assert_eq!(ground_known_name("Mirela Aldaravi", &hay, &known), None);
    }

    #[test]
    fn roster_lists_every_saved_entity() {
        let chars = vec!["Leonardo".to_string(), "Charlotte Bessa".to_string()];
        let r = roster_block(&chars, &["Aldébaran".to_string()], &[]);
        assert!(r.contains("Leonardo; Charlotte Bessa"));
        assert!(r.contains("Aldébaran"));
        assert!(roster_block(&[], &[], &[]).is_empty());
    }

    #[test]
    fn duplicate_name_forms_collapse_to_one_canonical() {
        let names = vec![
            "Leonardo".to_string(),
            "Leo".to_string(),
            "Rei Yan Serafine".to_string(),
            "Yan Serafine".to_string(),
        ];
        let m = canonical_map(&names);
        assert_eq!(m.get("leo").map(String::as_str), Some("Leonardo"));
        // The honorific form folds onto the plain name, not the other way around.
        assert_eq!(m.get("rei yan serafine").map(String::as_str), Some("Yan Serafine"));
    }

    #[test]
    fn ambiguous_short_form_is_left_alone() {
        let names = vec!["Leo".to_string(), "Leonardo".to_string(), "Leopoldo".to_string()];
        assert!(canonical_map(&names).get("leo").is_none());
    }

    #[test]
    fn nicknames_reach_the_dedup_pass() {
        // "Leo" shares no whole token with "Leonardo Silva", and "Lô" shares none
        // with "Charlotte" — both must still be offered to the LLM dedup.
        let names = vec!["Leonardo Silva".to_string(), "Leo".to_string(), "Lô".to_string(), "Charlotte".to_string()];
        let d = dubious_candidates(&names);
        assert!(d.contains(&"Leo".to_string()));
        assert!(d.contains(&"Lô".to_string()));
    }

    #[test]
    fn chapters_sort_in_reading_order() {
        let mut docs = vec!["Capítulo 10.txt", "Capítulo 2.txt", "Capítulo 1.txt", "Capítulo 21.txt"];
        docs.sort_by(|a, b| cmp_doc(a, b));
        assert_eq!(docs, vec!["Capítulo 1.txt", "Capítulo 2.txt", "Capítulo 10.txt", "Capítulo 21.txt"]);
    }

    #[test]
    fn first_occurrence_questions_are_detected() {
        assert!(wants_first_time("O que a Charlotte falou pela primeira vez para o Leonardo?"));
        assert!(wants_first_time("Quando eles se conheceram?"));
        assert!(!wants_first_time("O que a Charlotte falou no capítulo 5?"));
        // The old opening heuristic never fired for this class of question.
        assert!(!wants_opening("O que a Charlotte falou pela primeira vez para o Leonardo?"));
    }

    #[test]
    fn earliest_co_occurrence_is_retrieved() {
        let mk = |doc: &str, ord: usize, text: &str| Chunk {
            id: format!("{doc}-{ord}"),
            doc_id: doc.to_string(),
            doc_name: doc.to_string(),
            ordinal: ord,
            text: text.to_string(),
            vector: vec![],
        };
        let chunks = vec![
            mk("Capítulo 10.txt", 0, "Charlotte e Leonardo discutiram de novo."),
            mk("Capítulo 2.txt", 0, "Charlotte encarou Leonardo pela primeira vez."),
            mk("Capítulo 3.txt", 0, "Leonardo estava só."),
        ];
        let names = vec!["Charlotte".to_string(), "Leonardo".to_string()];
        let got = first_mention_chunks(
            "O que a Charlotte falou pela primeira vez para o Leonardo?",
            &names,
            &chunks,
            5,
        );
        // Chapter 2 must come first — plain string sort would have put 10 ahead of it.
        assert_eq!(got[0].doc_name, "Capítulo 2.txt");
    }

    #[test]
    fn dedup_canonical_must_exist_in_the_list() {
        // Simulates the model answering with an invented canonical name.
        let names = vec!["Leonardo".to_string(), "Leo".to_string()];
        let known: std::collections::HashMap<String, String> =
            names.iter().map(|n| (n.to_lowercase(), n.clone())).collect();
        let members: Vec<String> = ["Leo", "Leonardo"]
            .iter()
            .filter_map(|n| known.get(&n.to_lowercase()).cloned())
            .collect();
        let invented = "Leonardo Venante";
        assert!(known.get(&invented.to_lowercase()).is_none());
        let fallback = members
            .iter()
            .min_by_key(|n| (name_tokens(n).len(), n.chars().count()))
            .cloned()
            .unwrap();
        assert_eq!(fallback, "Leo");
    }
}
