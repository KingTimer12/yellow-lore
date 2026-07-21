# Yellow Lore — arquitetura

App de **chat de IA com RAG**: os documentos que você carrega são a base de
dados. O agente **sempre busca na base primeiro**, depois responde citando os
trechos-fonte.

## Stack

- **SolidJS** + Vite + **UnoCSS** (presetWind3). Tokens de tema como CSS vars
  (`src/theme.ts`) → dark/light trocam sozinhos.
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

## Backend Rust (`src-tauri/src/`)

- `db.rs` — **SQLite (rusqlite bundled)**: vaults, documents, chunks (vetor como
  JSON), characters, places, relations. Substitui o ChromaDB (que exige servidor
  Python) e o antigo `vectors.json`.
- `vector_store.rs` — só a matemática: cosseno + busca top-k sobre os chunks.
- `providers.rs` — embedding + chat via **Ollama** (local) e **OpenAI**,
  escolhidos de forma independente.
- `rag.rs` — chunking · `build_document()` (chunk→embed) · `ask()` = pipeline
  **RAG-first** · `extract_entities()` = LLM lê o vault e devolve JSON de
  personagens/lugares/relações.
- `config.rs` — `RagConfig` (`config.json`, global).
- `lib.rs` — estado + comandos Tauri.

### Comandos Tauri

Config: `get_config`, `save_config`. Vaults: `list_vaults`, `get_active_vault`,
`set_active_vault`, `create_vault`, `rename_vault`, `delete_vault`. Docs:
`list_documents`, `ingest_document`, `remove_document`. Chat: `ask`. Entidades:
`get_entities`, `extract_entities`, `update_character`, `update_place`.

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
- Ingestão hoje: **.txt / .md** (lidos no front via `file.text()`).

## Ainda mock / próximos passos

- **PDF / DOCX**: parsing ainda não; adicionar (crate `pdf-extract`, `docx-rs`)
  ou extrair texto no front antes do `ingest_document`. Hoje: .txt / .md.
- Streaming de resposta do LLM (hoje é resposta única).
- Extração roda sobre ~12k chars da base por vez; para obras grandes, iterar em
  janelas e mesclar entidades.
