# Yellow Lore — arquitetura

App de **chat de IA com RAG**: os documentos que você carrega são a base de
dados. O agente **sempre busca na base primeiro**, depois responde citando os
trechos-fonte.

## Stack

- **SolidJS** + Vite + **UnoCSS** (presetWind3). Tokens de tema como CSS vars
  (`src/theme.ts`) → dark/light trocam sozinhos.
- **Direção visual "Amber Codex"**: acento ouro âmbar (o "Yellow" literal) sobre
  tinta quente (dark) / pergaminho (light). Tipografia de manuscrito — Cinzel
  (marca/`font-display`), Cormorant Garamond (títulos/`font-serif`), Crimson Pro
  (leitura das respostas/`font-reading`), Inter (UI). Fontes via Google Fonts
  com fallback Georgia (offline mantém o feel).
- **Tauri (Rust)** faz todo o RAG e a chamada aos provedores.
- Estado central no front: `createStore` em `src/store.ts`.
- Ponte front↔Rust: `src/api.ts`. Fora do Tauri (`bun run dev` no navegador)
  cai em **mock** pra preview de design.

## Vaults (SQLite)

Cada **vault** = base de conhecimento isolada (uma obra/história). Tudo —
documentos, chunks, personagens, lugares, relações — é escopado por `vault_id`
em **SQLite** (`yellow-lore.db` no app data dir, via `rusqlite` bundled — nada
extra pra instalar). O vault ativo fica na tabela `meta`. Troca de vault no
seletor da sidebar.

**Início vazio (estilo Obsidian)**: o app abre **sem nenhum vault**;
`get_active_vault` devolve `null` e o front mostra a tela "Criar vault". Não há
vault padrão automático. O chat também começa vazio e troca de vault limpa a
conversa.

**Memória de chat**: `ask` recebe o histórico inteiro da conversa (`history`)
do front e o repassa ao LLM, então o assistente mantém contexto entre turnos.

**Sessões de chat (estilo ChatGPT)**: tabelas `sessions` + `messages` (por
vault). Cada conversa é uma sessão salva; o app abre numa conversa **em branco**
(sem sessão) e a sessão é criada na primeira mensagem (título provisório = 1ª pergunta, depois
substituído por um **resumo curto gerado por LLM** via `generate_session_title`).
Histórico persiste e nunca se perde. Rail de conversas no ChatView (nova, abrir,
renomear, excluir). Comandos: `list_sessions`, `create_session`,
`rename_session`, `delete_session`, `session_messages`, `add_message`.

## Backend Rust (`src-tauri/src/`)

- `db.rs` — **SQLite (rusqlite bundled, WAL)**: vaults, documents, chunks,
  characters, places, relations. Vetores gravados como **BLOB f32 little-endian**
  (4 bytes/dim — menor e mais rápido que JSON; leitura aceita o formato JSON
  antigo). `id` do documento é **BLAKE3(conteúdo)** → reingestão idempotente e
  dedupe. Meta `emb:<vault>` guarda o modelo de embedding indexado (p/ detectar
  índice desatualizado). Substitui o ChromaDB e o antigo `vectors.json`.
- `vector_store.rs` — a matemática: cosseno + busca top-k, **+ `keyword_search`
  lexical com peso IDF** (termo raro vence palavras comuns) para a **busca
  híbrida** (semântico + lexical), cobrindo match literal que o embedding perde.
- `providers.rs` — embedding + chat via **Ollama** (local), **OpenAI** e
  **vLLM** (servidor OpenAI-compatível, key opcional), escolhidos de forma
  independente.
- `rag.rs` — chunking · `build_document()` (chunk→embed) · `ask()` = pipeline
  **RAG-first** · `ask_stream()` = mesmo pipeline, mas emite tokens via callback.
  Recuperação: **híbrida** (semântico + lexical IDF), **direcionada por capítulo**
  (pergunta "capítulo 1" puxa só aquele doc em ordem de leitura), **GraphRAG-lite**
  (`graph_context`: detecta entidades citadas na pergunta e injeta o subgrafo de
  relações ao redor — seeds + 1 hop — como fatos estruturados; resolve perguntas
  multi-hop "quem é o mestre de X" que o cosseno erra), **injeção da
  abertura** (`ordinal 0`) para perguntas posicionais, **reranking opcional**
  (`rerank`: uma chamada de LLM reordena os trechos por relevância antes de montar
  o contexto) e **citações declaradas pelo LLM**: cada trecho vai rotulado
  `[Fonte N]` e o modelo cita `[N]`; `cited_sources` mantém só as fontes marcadas,
  com **fallback** ao filtro de sobreposição (`relevant_sources`) quando o modelo
  não marca nada. Chunks têm `ordinal` (ordem de leitura, recalculado no load).
  `extract_entities()` = LLM lê o vault em **janelas de ~12k chars** (até **40**
  janelas), com **concorrência configurável** (`extractionConcurrency`, default 1
  = sequencial; suba só em nuvem), **modelo de extração dedicado opcional**
  (`extractionModel`; vazio = reutiliza o de chat), e **mescla** as entidades por
  nome (case-insensitive; traits unidos, relações dedup). **Incremental**: só
  documentos ainda não extraídos (set em
  `meta`); entidades **Editado/Adicionado nunca são sobrescritas** (`is_protected`).
  Extração de JSON tolerante a `<think>` (strip antes de parsear). Coreferência:
  alias por subsequência de tokens ("Cesar" → "Cesar Magnus") + **dedup opcional
  via LLM** (`dedupEntities`) **direcionado** (só nomes ambíguos, que compartilham
  algum token) e **contra entidades já salvas** (funde "Cesar" de um run novo na
  "Cesar Magnus" de um run anterior). Nomes canônicos reescrevem também as relações.
- `config.rs` — `RagConfig` (`config.json`, global).
- `lib.rs` — estado + comandos Tauri.

**Relações manuais (curadoria do grafo)**: o usuário adiciona/remove arestas na
aba Grafo (`RelationsEditor` em `CharactersView`). Comandos `add_relation` /
`remove_relation` (chave natural `from,to,label`, case-insensitive; `reset_extracted`
nunca apaga relações). Isso alimenta o GraphRAG — como a extração automática fica
mais imprecisa a cada capítulo novo, ligações curadas à mão mantêm a recuperação
correta.

### Comandos Tauri

Config: `get_config`, `save_config`. Vaults: `list_vaults`, `get_active_vault`,
`set_active_vault`, `create_vault`, `rename_vault`, `delete_vault`. Docs:
`list_documents`, `ingest_document`, `ingest_binary`, `remove_document`,
`index_info`, `reindex`. Chat: `ask`, `ask_stream` (tokens via `Channel`; front
separa `<think>` e renderiza markdown), `cancel_generation` (para a geração via
`AtomicBool`). Entidades: `get_entities`, `extract_entities` (arg `force`:
incremental por padrão, `true` re-scaneia tudo), `add_character`, `add_place`,
`update_character`, `update_place`.

## Config (Settings) — LLM ≠ embedding

- **LLM**: provedor (Ollama/OpenAI) + modelo.
- **Embedding**: provedor (Ollama/OpenAI) + modelo — separado do LLM.
- Credenciais: OpenAI (key + base URL) e/ou Ollama (endpoint).
- **System prompt** editável (esteira o agente).
- RAG: chunk size, overlap, top-k, **temperatura** (default 0.2), **num_ctx do
  Ollama** (default 8192 — modelo que raciocina muito estourava o contexto e
  cortava a resposta), mostrar fontes, **reranking** (off).
- Extração: **modelo dedicado** opcional (`extractionModel`), **janelas em
  paralelo** (`extractionConcurrency`, default 1), **dedup via LLM**
  (`dedupEntities`).

## Rodar

- `bun run dev` → navegador, **mock** (sem Rust/IA real).
- `bun run tauri dev` → app real. Requer:
  - **Ollama** rodando (`ollama serve`) + modelos (`ollama pull llama3.1`,
    `ollama pull nomic-embed-text`), **ou**
  - chave OpenAI no Settings.
- Ingestão: **.txt / .md** (lidos no front via `file.text()`) e **.pdf / .docx**
  (front envia bytes em base64; `extract.rs` extrai o texto no Rust — `pdf-extract`
  para PDF, `zip` + `quick-xml` lendo `word/document.xml` para DOCX).

## Ainda mock / próximos passos

- Grafo (`Graph.tsx`): força-dirigido no front (SVG, sem lib), não usa layout
  do backend; para vaults enormes (centenas de nós) trocar a repulsão O(n²) por
  Barnes-Hut.

## Limitações conhecidas (levantadas em uso real)

- **Truncamento de raciocínio**: mesmo com `num_ctx` maior, modelo muito
  verboso ainda pode estourar. Modelo não-reasoning ou `num_ctx` maior ajuda.
- **macOS**: sem Apple Developer ID, o auto-update funciona mas o SO alerta app
  não-notarizado. (Sem solução do meu lado — exige conta paga da Apple.)

### Resolvidas

- ~~Dedup de entidades entre extrações~~: o dedup-LLM agora roda **contra as
  entidades já salvas** ("Cesar" de um run novo funde na "Cesar Magnus" salva).
- ~~Filtro de citações heurístico~~: substituído por **citações declaradas pelo
  LLM** (`[N]` → `[Fonte N]`), com fallback ao filtro de sobreposição.
- ~~Primeira extração de obra grande é lenta~~: janelas agora rodam com
  **concorrência configurável** (para nuvem). Local segue sequencial de propósito
  (paralelizar numa GPU só estoura a VRAM).

## Ideias — precisão da resposta (chat)

- **RAG avançado / CRAG (Corrective RAG)**: após gerar, o modelo se auto-avalia
  ("isso responde?"); se não, descarta as fontes fracas e re-busca uma vez antes
  de revisar a resposta. Opcional (custa +1..2 chamadas — pesa em local), bom para
  o modo precisão. Limitar a 1 re-busca (nada de loop aberto estilo agentic puro).
- **Fusão de rankings (RRF)**: combinar semântico e lexical por Reciprocal Rank
  Fusion em vez de append+dedup — ordena melhor quando os dois discordam.
- **Threshold de similaridade**: descartar chunk abaixo de X de cosseno (menos
  contexto-ruído; hoje entra tudo do top-k).
- **Query rewrite / HyDE**: reescrever a pergunta ou gerar uma resposta
  hipotética e embedá-la → melhora recall de perguntas mal formuladas.
- **Janela de vizinhança**: junto do chunk relevante, incluir `ordinal±1` do
  mesmo doc → dá continuidade de leitura ao modelo.
- **Metadados estruturais no chunk**: guardar nº/título de capítulo/seção para
  filtrar por estrutura (hoje a inferência de capítulo é pelo nome do arquivo).
- **Limpeza de PDF/DOCX**: remover cabeçalho/rodapé/numeração de página que
  entram como ruído no texto extraído.

## Ideias — tempo de pesquisa (chat)

- **Manter o modelo quente**: `keep_alive` no Ollama para não recarregar o
  modelo a cada chamada (grande parte do tempo em cold start).
- **Índice vetorial aproximado (HNSW/IVF)** quando o vault crescer — hoje a
  busca é brute-force O(n) sobre todos os chunks. Ok pessoal, mas 12+ capítulos
  já pesa.
- **Cache de embedding de perguntas** repetidas/parecidas.
- **top-k e contexto menores** = menos tokens no prompt = resposta mais rápida
  (equilibrar com recall).

## Ideias — precisão da extração

- **Saída estruturada garantida**: Ollama `format: "json"` / grammar GBNF ou
  JSON schema → elimina falha de "JSON inválido" de modelos pequenos.
- **Few-shot com personagens conhecidos**: injetar nomes canônicos já extraídos
  no prompt → consistência de nomeação entre janelas/runs.
- Feito: **dedup contra entidades existentes** e **verificação direcionada de
  entidades duvidosas** (só nomes ambíguos vão ao dedup-LLM).

## Ideias — tempo de extração

- Feito: **paralelizar as janelas** (`extractionConcurrency`, só nuvem por
  padrão) e **modelo dedicado à extração** (`extractionModel`).
- **Pular dedup-LLM** quando há poucas entidades (já é barato, mas evita 1 call).
- **Cache por `doc_id` (BLAKE3)**: reingestão idêntica não re-extrai (o id já é
  content-addressed; falta ligar isso ao set de extraídos).
- **Incremental** (já implementado): só documentos novos são processados.
