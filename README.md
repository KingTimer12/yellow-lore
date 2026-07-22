# Yellow Lore

App desktop de **chat de IA com RAG**: seus documentos viram a base de
conhecimento e o assistente **sempre busca na base antes de responder**,
citando os trechos-fonte. Organize obras diferentes em **vaults** isolados e
extraia **personagens, lugares e habilidades** automaticamente da base, com
um **grafo de relações** editável.

Stack: **SolidJS + UnoCSS + Tauri (Rust)**. Armazenamento local em **SQLite**;
nada de servidores externos ou Python.

## Funcionalidades

- **Chat RAG** — busca híbrida (semântica + lexical) e responde citando
  **apenas** as fontes que a resposta usou.
- **Base de conhecimento** — ingestão de `.txt`, `.md`, `.pdf` e `.docx`.
- **Vaults** — cada obra/história é uma base isolada (SQLite). App inicia sem
  vault; você cria o primeiro (estilo Obsidian).
- **Memória de conversa** — o chat inicia vazio e mantém o contexto de toda a
  conversa entre turnos.
- **Extração de entidades** — LLM identifica **personagens, lugares,
  habilidades** e relações, decidindo pelo contexto. **Incremental**: só
  processa documentos novos; adicionar capítulos não reprocessa os antigos.
- **Curadoria protegida** — adicione ou edite entidades e relações à mão;
  o que você toca (**Editado/Adicionado/Manual**) **nunca** é sobrescrito nem
  apagado por uma nova extração. Exclua entidades e limpe as relações órfãs.
- **Grafo de relações** — personagens, lugares e habilidades num só mapa;
  crie arestas ligando nós pelo painel, edite o rótulo e exclua.
- **Busca e ordenação** — filtre por nome/tipo/traço; listas em ordem
  alfabética.
- **Provedores** — Ollama (local), OpenAI e vLLM (servidor OpenAI-compatível);
  modelo de LLM e de embedding configurados de forma **independente**.
- **RAG configurável** — chunk size, overlap, top-k, temperatura, num_ctx,
  prompt do sistema.
- **Auto-update** (desktop) via Tauri updater a partir do GitHub Release.
- Tema claro/escuro, escala da UI conforme a tela, micro-animações,
  tudo offline-first.

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
