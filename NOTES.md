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
- `vector_store.rs` — só a matemática: cosseno + busca top-k sobre os chunks.
- `providers.rs` — embedding + chat via **Ollama** (local), **OpenAI** e
  **vLLM** (servidor OpenAI-compatível, key opcional), escolhidos de forma
  independente.
- `rag.rs` — chunking · `build_document()` (chunk→embed) · `ask()` = pipeline
  **RAG-first** · `ask_stream()` = mesmo pipeline, mas emite tokens via callback ·
  `extract_entities()` = LLM lê o vault em **janelas de ~12k chars** (até 12
  janelas) e **mescla** as entidades por nome (case-insensitive; traits unidos,
  relações dedup) → cobre obras grandes, não só o começo. Coreferência: alias por
  subsequência de tokens ("Cesar" → "Cesar Magnus") + **dedup opcional via LLM**
  (`dedupEntities`, uma chamada extra que agrupa apelidos/títulos). Nomes
  canônicos reescrevem também as relações do grafo.
- `config.rs` — `RagConfig` (`config.json`, global).
- `lib.rs` — estado + comandos Tauri.

### Comandos Tauri

Config: `get_config`, `save_config`. Vaults: `list_vaults`, `get_active_vault`,
`set_active_vault`, `create_vault`, `rename_vault`, `delete_vault`. Docs:
`list_documents`, `ingest_document`, `ingest_binary`, `remove_document`,
`index_info`, `reindex`. Chat: `ask`. Entidades:
`get_entities`, `extract_entities`, `update_character`, `update_place`. Chat
streaming: `ask_stream` (emite tokens via `Channel`; front separa `<think>` e
renderiza markdown).

## Config (Settings) — LLM ≠ embedding

- **LLM**: provedor (Ollama/OpenAI) + modelo.
- **Embedding**: provedor (Ollama/OpenAI) + modelo — separado do LLM.
- Credenciais: OpenAI (key + base URL) e/ou Ollama (endpoint).
- **System prompt** editável (esteira o agente).
- RAG: chunk size, overlap, top-k, mostrar fontes.

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
