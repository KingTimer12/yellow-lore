# Yellow Lore

App desktop de **chat de IA com RAG**: seus documentos viram a base de
conhecimento e o assistente **sempre busca na base antes de responder**,
citando os trechos-fonte. Organize obras diferentes em **vaults** isolados e
extraia **personagens e lugares** automaticamente da base.

Stack: **SolidJS + UnoCSS + Tauri (Rust)**. Armazenamento local em **SQLite**;
nada de servidores externos ou Python.

## Funcionalidades

- **Chat RAG** — recupera os trechos mais relevantes e responde com fontes.
- **Base de conhecimento** — ingestão de `.txt` / `.md` (PDF/DOCX no roadmap).
- **Vaults** — cada obra/história é uma base isolada (SQLite).
- **Extração de entidades** — LLM identifica personagens, lugares e relações.
- **Provedores** — Ollama (local) e OpenAI; modelo de LLM e de embedding
  configurados de forma **independente**.
- **RAG configurável** — chunk size, overlap, top-k, prompt do sistema.
- Tema claro/escuro, micro-animações, tudo offline-first.

## Rodar

```bash
bun install
bun run dev        # preview no navegador (dados mock) — http://localhost:1420
bun run build      # build de produção do frontend
bun run tauri dev  # app desktop real (RAG + SQLite + provedores)
```

`bun run dev` roda só o frontend com dados mock (sem Rust). Para o RAG real use
`bun run tauri dev` e configure um provedor:

- **Ollama** (local): `ollama serve` + `ollama pull llama3.1` +
  `ollama pull nomic-embed-text`, ou
- **OpenAI**: informe a API key em Configurações.

## Requisitos

- [Bun](https://bun.sh) e [Rust](https://rustup.rs) + toolchain do
  [Tauri](https://tauri.app) (para `tauri dev`).

## Arquitetura

Detalhes de backend, comandos Tauri e roadmap em [NOTES.md](./NOTES.md).

## Licença

[MIT](./LICENSE).
