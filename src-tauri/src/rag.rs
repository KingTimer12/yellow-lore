use crate::config::RagConfig;
use crate::db::{Character, Place, Relation};
use crate::error::{AppError, AppResult};
use crate::providers::{self, ChatMessage};
use crate::vector_store::{self, Chunk};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub doc: String,
    pub quote: String,
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

fn short_quote(text: &str) -> String {
    let trimmed = text.trim();
    let mut q: String = trimmed.chars().take(160).collect();
    if trimmed.chars().count() > 160 {
        q.push('…');
    }
    format!("\"{q}\"")
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
    let doc_id = Uuid::new_v4().to_string();
    let vectors = if pieces.is_empty() {
        Vec::new()
    } else {
        providers::embed(client, cfg, &pieces).await?
    };
    let chunks = pieces
        .into_iter()
        .zip(vectors)
        .map(|(text, vector)| Chunk {
            id: Uuid::new_v4().to_string(),
            doc_id: doc_id.clone(),
            doc_name: name.clone(),
            text,
            vector,
        })
        .collect();
    Ok(BuiltDoc { kind: file_kind(&name), id: doc_id, name, chunks })
}

// ---- Ask (RAG-first) ------------------------------------------------------

pub async fn ask(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
    question: String,
) -> AppResult<Answer> {
    let mut sources: Vec<Source> = Vec::new();
    let mut context = String::new();

    if !chunks.is_empty() {
        let qvec = providers::embed(client, cfg, &[question.clone()]).await?;
        let hits = vector_store::search(chunks, &qvec[0], cfg.top_k);
        for (i, hit) in hits.iter().enumerate() {
            context.push_str(&format!(
                "[{}] Documento: {}\n{}\n\n",
                i + 1,
                hit.chunk.doc_name,
                hit.chunk.text
            ));
            sources.push(Source {
                doc: hit.chunk.doc_name.clone(),
                quote: short_quote(&hit.chunk.text),
            });
        }
    }

    let context_block = if context.is_empty() {
        "(nenhum documento indexado neste vault ou nenhum trecho relevante)".to_string()
    } else {
        context
    };
    let system = format!(
        "{}\n\nContexto recuperado da base de conhecimento:\n{}",
        cfg.system_prompt, context_block
    );
    let messages = vec![
        ChatMessage { role: "system", content: system },
        ChatMessage { role: "user", content: question },
    ];
    let text = providers::chat(client, cfg, &messages).await?;

    sources.dedup_by(|a, b| a.doc == b.doc && a.quote == b.quote);
    Ok(Answer { text, sources })
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
    relations: Vec<ExtractedRel>,
}

/// Ask the LLM to read the vault's knowledge and extract characters, places and
/// relations as structured JSON.
pub async fn extract_entities(
    client: &reqwest::Client,
    cfg: &RagConfig,
    chunks: &[Chunk],
) -> AppResult<(Vec<Character>, Vec<Place>, Vec<Relation>)> {
    if chunks.is_empty() {
        return Err(AppError::Msg(
            "vault sem documentos indexados — carregue algo primeiro".into(),
        ));
    }

    // Bound the context so the prompt stays reasonable.
    let mut corpus = String::new();
    for c in chunks {
        if corpus.len() > 12000 {
            break;
        }
        corpus.push_str(&format!("[{}]\n{}\n\n", c.doc_name, c.text));
    }

    let system = "Você extrai entidades de textos de ficção/worldbuilding. \
Responda APENAS com JSON válido, sem texto extra, sem markdown. Formato:\n\
{\"characters\":[{\"name\":\"\",\"role\":\"\",\"summary\":\"\",\"traits\":[\"\"],\"sourceDoc\":\"\",\"sourceQuote\":\"\"}],\
\"places\":[{\"name\":\"\",\"type\":\"\",\"summary\":\"\",\"sourceDoc\":\"\",\"sourceQuote\":\"\"}],\
\"relations\":[{\"from\":\"\",\"to\":\"\",\"label\":\"\"}]}\n\
sourceQuote deve ser uma citação curta e literal do texto. Use o idioma do texto.";

    let user = format!(
        "Extraia personagens, lugares e relações do conhecimento abaixo:\n\n{corpus}"
    );

    let messages = vec![
        ChatMessage { role: "system", content: system.to_string() },
        ChatMessage { role: "user", content: user },
    ];
    let raw = providers::chat(client, cfg, &messages).await?;
    let json = extract_json_block(&raw)
        .ok_or_else(|| AppError::Msg("modelo não retornou JSON válido para extração".into()))?;
    let parsed: Extraction = serde_json::from_str(&json)
        .map_err(|e| AppError::Msg(format!("falha ao ler JSON da extração: {e}")))?;

    let characters = parsed
        .characters
        .into_iter()
        .map(|c| Character {
            id: Uuid::new_v4().to_string(),
            name: c.name,
            role: c.role,
            summary: c.summary,
            traits: c.traits,
            status: "Extraído".into(),
            source_doc: c.source_doc,
            source_quote: c.source_quote,
        })
        .collect();

    let places = parsed
        .places
        .into_iter()
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

    let relations = parsed
        .relations
        .into_iter()
        .map(|r| Relation { from: r.from, to: r.to, label: r.label })
        .collect();

    Ok((characters, places, relations))
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
