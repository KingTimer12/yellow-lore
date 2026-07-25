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
  characters, places, **abilities**, relations (com `status`
  Manual/Extraído, migrado por `ALTER TABLE` na abertura). `delete_entity` remove a
  linha **e as relações órfãs** dela, senão sobra vértice morto no grafo.
  `messages.duration_ms` guarda o tempo de cada resposta (NULL nas linhas antigas,
  que a UI trata como "sem tempo", não como zero). Vetores gravados como **BLOB f32 little-endian**
  (4 bytes/dim — menor e mais rápido que JSON; leitura aceita o formato JSON
  antigo). `id` do documento é **BLAKE3(conteúdo)** → reingestão idempotente e
  dedupe. Meta `emb:<vault>` guarda o modelo de embedding indexado (p/ detectar
  índice desatualizado). Substitui o ChromaDB e o antigo `vectors.json`.
- `vector_store.rs` — a matemática: cosseno + busca top-k, **+ `keyword_search`
  lexical com peso IDF** (termo raro vence palavras comuns) para a **busca
  híbrida** (semântico + lexical), cobrindo match literal que o embedding perde.
- `providers.rs` — embedding + chat via **Ollama** (local), **OpenAI** e
  **vLLM** (servidor OpenAI-compatível, key opcional), escolhidos de forma
  independente. **Thinking só na resposta final**: `chat()` (resposta ao usuário)
  deixa o raciocínio ligado; `chat_internal()` (rerank, grade do CRAG, dedup,
  extração, título) desliga — no Ollama envia `think:false`, e os prompts internos
  ainda carregam o hint `/no_think` p/ modelos OpenAI/vLLM. Corta latência dos
  passos internos que só devolvem saída curta/estruturada. A **resposta final**
  também não raciocina por padrão (`showThinking` off — evita que modelos que
  "pensam em texto puro" vazem um preâmbulo na resposta); ligável nas Configurações.
  O **CRAG sempre transmite** a resposta final por streaming — o rascunho é só
  checagem interna barata (no-think), nunca o que o usuário vê.
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
  não marca nada. **RAG corretivo opcional** (`corrective` / CRAG-limitado):
  rascunha a resposta, `grade_answer` avalia se ela resolve a pergunta; se não,
  re-busca com rede mais ampla (`widen`: top-k dobrado, capado) e responde uma vez
  mais. Uma única re-tentativa (sem loop aberto). No streaming: rascunho adequado é
  entregue direto (um push); só a re-tentativa é transmitida token a token.
  Chunks têm `ordinal` (ordem de leitura, recalculado no load).
  **Ordem de leitura natural** (`cmp_doc`): nomes de documento são comparados
  cientes de número, senão "Capítulo 10" vem antes de "Capítulo 2" e a ordem
  quebra a partir de dez capítulos. **Perguntas de primeira ocorrência**
  (`wants_first_time`: "primeira vez", "quando se conheceram") injetam os trechos
  mais **antigos** que citam as entidades da pergunta — o cosseno não tem noção de
  "mais antigo" e caía em capítulo tardio. O system prompt **proíbe dedução**:
  responder só o que está escrito, dizer o que falta, nunca supor vínculo anterior
  que o texto não afirma.

### Extração de entidades (`extract_entities`)

Personagens, **lugares** e **habilidades** (poderes/magias/técnicas — "Previsão",
"Espadas do Julgamento") mais as relações entre eles. O pipeline é map-reduce: as
janelas extraem candidatos, tudo é conciliado **em memória**, e só então
`merge_extracted` grava uma vez — nada é gravado janela a janela.

- **Janelas** de ~12k chars (até **150**; 40 já cortava ~40% de uma obra de 29
  capítulos em silêncio, junto dos personagens dos capítulos finais). A janela
  **quebra em fronteira de capítulo** quando já tem ≥4k chars, para não colar o fim
  de um capítulo no começo do outro; capítulos curtos compartilham janela para não
  gastar uma chamada cada (`build_windows`).
- **Registro corrente no prompt**: cada janela recebe a lista de **todas** as
  entidades conhecidas (as salvas no vault **e** as descobertas nas janelas
  anteriores da mesma rodada), com a instrução de copiar o nome letra por letra e
  só acrescentar o que o texto novo revela. As janelas rodam em **lotes do tamanho
  de `extractionConcurrency`** e a lista é remontada entre lotes — o registro
  cresce sem nenhuma chamada de LLM extra e o paralelismo de nuvem é preservado.
  Como `merge_extracted` atualiza por nome, reusar o nome exato é o que transforma
  duplicata em atualização.
- **Grounding** (`ground_extraction`): o modelo não é acreditado. Cada nome é
  reescrito para a maior sequência das próprias palavras que aparece
  **literalmente** (com fronteira de palavra) na janela lida; sem forma ancorada, a
  entidade é descartada. Mata personagem/habilidade inventado e **sobrenome
  fabricado** — "Leonardo Venante" ("Venante" é o nome da classe com poderes, nunca
  grudado nele no texto) colapsa em "Leonardo", junto de "Leo / Leonardo" e
  "Leonardo (Leo) Venante". `sourceQuote` que não existe no texto é apagada. Se o
  nome bate com uma entidade conhecida, a **grafia salva** vence (capítulo escreve
  "Charlotte", card é "Charlotte Bessa" → "Charlotte Bessa"), mas a evidência
  continua obrigatória: nome do registro que a janela não menciona é descartado,
  senão o modelo copiaria a lista inteira para a saída.
- **Termos relacionais**: nome não pode começar com preposição ("de Leonardo",
  fatiado de "a mãe de Leonardo", virava vértice ao lado do Leonardo real) e
  substantivo de papel/parentesco como cabeça do nome ("mãe", "a mãe de Leonardo",
  "a madrasta") deixa de ser entidade — duplicava a pessoa nomeada ("mãe" ao lado
  de "Elisa"). Só passa quando o texto usa a palavra como **nome próprio**:
  maiúscula no meio da frase (`appears_as_proper_noun`), o que distingue "A Bruxa"
  de "a mãe". O vínculo sobrevive como aresta rotulada. Artigos são preservados.
- **Prompt**: nome copiado exatamente como está escrito (nunca inventar sobrenome
  nem colar palavras vizinhas — título, classe, espécie e facção não são
  sobrenome), um nome por entrada, papel genérico vira relação e não entidade,
  traits com no máximo 6 tags de uma palavra (o resto vai para o summary) e
  **proibido resumo com alternativas** ("madrasta ou mãe biológica") — decidir pelo
  contexto ou descrever só o que o texto afirma. Habilidade exige contexto de
  poder: arma nomeada ("uma espada sagrada") e organização ("a Inquisição") não
  contam, e capítulo inicial sem nenhuma é normal.
- **Coreferência** (`canonical_map`): contenção de tokens **sem ordem** — a versão
  ordenada não casava `[leo, leonardo]` com `[leonardo, leo, venante]` —, títulos
  colapsando na forma simples ("Rei Yan Serafine" → "Yan Serafine") e diminutivos
  ("Leo" → "Leonardo") só quando existe uma única forma longa possível. Inclui os
  nomes já salvos, então um nome novo funde no card antigo. Nomes canônicos
  reescrevem também as relações.
- **Dedup com contexto** (`dedupWithContext`, **off** por padrão): manda também o
  resumo de uma linha (cortado em 140 chars) e até 3 relações diretas de cada
  candidato. É a única forma de pegar duplicata cujos nomes não têm nada em comum,
  mas infla a carga daquela chamada cerca de 10× (≈1,1k → ≈15k chars num lote de 60)
  e fundir duas pessoas diferentes é pior que deixar uma duplicata — daí ser opção,
  não comportamento fixo. O prompt ganha o aviso de que contexto parecido não basta
  (dois irmãos têm o mesmo papel).
- **Dedup opcional via LLM** (`dedupEntities`), direcionado: só nomes ambíguos —
  que compartilham token, ou são forma curta por prefixo ("Leo"), ou apelido curto
  ("Lô") — vão para o modelo, em **lotes de 60** (uma chamada com centenas de nomes
  truncava e alucinava). `canonical` e aliases **só valem se vierem da lista
  enviada**: sem essa checagem o modelo inventa o nome do grupo inteiro.
- **Vínculo pendente**: quando o lado relacional não é nomeado em lugar nenhum
  ("a mãe de Leonardo", ninguém nomeado), a aresta não é jogada fora — vira relação
  `status='Pendente'`, guardando o termo e o lado nomeado. Pendentes **não** entram
  em `list_relations`, logo não aparecem no grafo nem no GraphRAG: só na lista de
  curadoria, onde o usuário clica "Nomear" e ela se torna aresta `Manual`. A
  promoção **nunca é automática** — "mesmo alvo + mesmo rótulo" é fraco demais para
  apostar identidade (um personagem pode ter duas tias) e o projeto já se queimou
  com fusão silenciosa errada. O termo precisa existir literalmente no texto, senão
  nem pendente é gravado.
- **Incremental**: só documentos ainda não extraídos (set em `meta`). Entidades
  **Editado/Adicionado** e relações **Manual**/**Pendente** nunca são sobrescritas
  nem apagadas (`is_protected`; `apply_aliases` só reescreve `status='Extraído'`).
  JSON tolerante a `<think>`. `extractionModel` opcional (vazio = reutiliza o de
  chat).
- **Progresso e cancelamento**: a cada lote fechado o backend emite
  `ExtractEvent::Progress` por `Channel` (janela N/total, contagem de
  personagens/lugares/habilidades, documentos do lote, tempo decorrido) — o limite
  de lote já existia para remontar o registro, então reportar não custa chamada
  nenhuma. `cancel_extraction` levanta um `AtomicBool` checado **entre** lotes,
  nunca no meio de uma janela (janela pela metade daria elenco parcial).
  Cancelar **mantém** tudo o que já foi conciliado e marca como extraído **somente
  os documentos lidos de ponta a ponta** (`covered_docs`), então a próxima rodada
  retoma os capítulos que faltaram em vez de pulá-los. A duração total vai para
  `meta` (`extract_stats:<vault>`) e a UI mostra "última extração: 4min 12s".
- `config.rs` — `RagConfig` (`config.json`, global).
- `lib.rs` — estado + comandos Tauri.

**Relações manuais (curadoria do grafo)**: o usuário adiciona/remove arestas na
aba Grafo (`RelationsEditor` em `CharactersView`) ou **ligando nós no próprio
grafo** — o botão no painel de edição fecha o drawer e o clique seguinte escolhe o
destino (`state.linkSource`), com rótulo editável e exclusão. Comandos
`add_relation` / `remove_relation` (chave natural `from,to,label`,
case-insensitive; `reset_extracted` nunca apaga relações). Aresta criada pelo
usuário nasce `status='Manual'` e a extração **nunca** a modifica. Isso alimenta o
GraphRAG — como a extração automática fica mais imprecisa a cada capítulo novo,
ligações curadas à mão mantêm a recuperação correta.

### Comandos Tauri

Config: `get_config`, `save_config`. Vaults: `list_vaults`, `get_active_vault`,
`set_active_vault`, `create_vault`, `rename_vault`, `delete_vault`. Docs:
`list_documents`, `ingest_document`, `ingest_binary`, `remove_document`,
`index_info`, `reindex`. Chat: `ask`, `ask_stream` (tokens via `Channel`; front
separa `<think>` e renderiza markdown), `cancel_generation` (para a geração via
`AtomicBool`). Entidades: `get_entities`, `extract_entities` (arg `force`:
incremental por padrão, `true` re-scaneia tudo — apaga só `status='Extraído'`),
`add_character`, `add_place`, `add_ability`, `update_character`, `update_place`,
`update_ability`, `delete_character`, `delete_place`, `delete_ability`,
`cancel_extraction` (para entre lotes, preservando o conciliado),
`last_extraction` (duração/data da última rodada), `promote_pending_relation`.
`extract_entities` recebe um `Channel` de progresso e devolve
`{entities, durationMs, cancelled}`.

## Config (Settings) — LLM ≠ embedding

- **LLM**: provedor (Ollama/OpenAI) + modelo.
- **Embedding**: provedor (Ollama/OpenAI) + modelo — separado do LLM.
- Credenciais: OpenAI (key + base URL) e/ou Ollama (endpoint).
- **System prompt** editável (esteira o agente).
- RAG: chunk size, overlap, top-k, **temperatura** (default 0.2), **num_ctx do
  Ollama** (default 8192 — modelo que raciocina muito estourava o contexto e
  cortava a resposta), mostrar fontes, **reranking** (off), **RAG corretivo /
  CRAG** (off).
- Extração: **modelo dedicado** opcional (`extractionModel`), **janelas em
  paralelo** (`extractionConcurrency`, default 1 — é também o tamanho do lote entre
  as remontagens do registro corrente), **dedup via LLM** (`dedupEntities`) e **contexto na unificação**
  (`dedupWithContext`, off).
- **Escala da UI**: `zoom` no `.app-shell` por largura de viewport (1.1 acima de
  1600px, 1.2 acima de 2100px, 1.32 acima de 2800px) — tela grande não deixa a
  fonte minúscula.

## Rodar

- `bun run dev` → navegador, **mock** (sem Rust/IA real).
- `bun run tauri dev` → app real. Requer:
  - **Ollama** rodando (`ollama serve`) + modelos (`ollama pull llama3.1`,
    `ollama pull nomic-embed-text`), **ou**
  - chave OpenAI no Settings.
- `cd src-tauri && cargo test --lib` → testes de unidade de `rag.rs`: grounding
  (sobrenome fabricado, entidade e citação inventadas), termos relacionais
  ("mãe"/"de Leonardo"/"A Bruxa"), coreferência (títulos, diminutivos, forma curta
  ambígua), registro corrente, quebra de janela por capítulo, ordem natural de
  capítulo e recuperação da primeira ocorrência. Roda sem LLM.
- Verificação manual da extração num vault grande: anote o estado, rode
  **"Tudo"** (re-extrair, apaga só `status='Extraído'`), e confira que papel
  genérico não é personagem, que o protagonista está num card só e que o resumo
  não tem alternativas ("X ou Y"). Depois adicione um capítulo e rode "Extrair
  novos": nada deve duplicar o que já existia.
- Ingestão: **.txt / .md** (lidos no front via `file.text()`) e **.pdf / .docx**
  (front envia bytes em base64; `extract.rs` extrai o texto no Rust — `pdf-extract`
  para PDF, `zip` + `quick-xml` lendo `word/document.xml` para DOCX).

## Ainda mock / próximos passos

- Grafo (`Graph.tsx`): força-dirigido no front (SVG, sem lib), não usa layout
  do backend. Personagens, lugares e habilidades no mesmo mapa, com **controles de
  volume** — busca de foco (o nó e sua vizinhança), filtro por tipo, "só
  conectados" e limite de nós desenhados (top N por conexões, default 60) com
  contagem do que está oculto; sem isso uma obra de 29 capítulos vira um borrão.
  Posições são salvas a cada frame, senão mudar filtro joga o layout de volta na
  espiral inicial. Um toggle **pendentes** (off, só aparece se houver algum)
  desenha os vínculos ainda sem nome como **nó fantasma tracejado** ligado por
  aresta tracejada; clicar no fantasma pergunta quem é e promove. Fica off por
  padrão para o mapa nunca sugerir um vínculo que a obra não nomeou, e o modo de
  ligar aresta se recusa a apontar para um fantasma — seria justamente o vértice
  morto que a extração passou a rejeitar. Para vaults enormes ainda vale trocar a repulsão O(n²) por
  Barnes-Hut.

## Limitações conhecidas (levantadas em uso real)

- **Truncamento de raciocínio**: mesmo com `num_ctx` maior, modelo muito
  verboso ainda pode estourar. Modelo não-reasoning ou `num_ctx` maior ajuda.
- **macOS**: sem Apple Developer ID, o auto-update funciona mas o SO alerta app
  não-notarizado. (Sem solução do meu lado — exige conta paga da Apple.)

### Resolvidas

- ~~Extração degradando com o volume (29 capítulos)~~: personagem fragmentado em
  vários vértices, sobrenome inventado, habilidade e personagem alucinados, e
  pedaço de frase ("de Leonardo") ou papel genérico ("mãe") como entidade. Causas
  reais, todas no código: nada validava o retorno do modelo; o prompt mandava "use
  sempre o nome mais completo" (o que fazia colar palavras vizinhas); a conciliação
  por subsequência era sensível à ordem; nome relacional nunca era rejeitado; e cada
  janela redescobria a mesma pessoa sem ver o que as outras já tinham achado. Ver a
  seção de extração acima.
- ~~"Capítulo 10" antes de "Capítulo 2"~~: ordenação de documento era comparação de
  string. Com dez capítulos ou mais a ordem de leitura quebrava e levava consigo
  qualquer pergunta de "primeira vez".
- ~~Pergunta de primeira ocorrência sem resposta~~: "o que X falou pela primeira vez
  para Y" não acionava nada (a heurística de abertura exigia "frase"/"parágrafo") e
  o top-k caía em capítulo tardio; o modelo então dizia não saber e supunha que os
  dois já se conheciam.
- ~~Últimos capítulos ignorados na extração~~: `MAX_WINDOWS` 40 (~480k chars) era
  estourado por 29 capítulos e o resto era descartado em silêncio.
- ~~Dedup de entidades entre extrações~~: o dedup-LLM agora roda **contra as
  entidades já salvas** ("Cesar" de um run novo funde na "Cesar Magnus" salva).
- ~~Filtro de citações heurístico~~: substituído por **citações declaradas pelo
  LLM** (`[N]` → `[Fonte N]`), com fallback ao filtro de sobreposição.
- ~~Primeira extração de obra grande é lenta~~: janelas agora rodam com
  **concorrência configurável** (para nuvem). Local segue sequencial de propósito
  (paralelizar numa GPU só estoura a VRAM).

## Ideias — precisão da resposta (chat)

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
  JSON schema → elimina falha de "JSON inválido" de modelos pequenos. **Cuidado**:
  já foi tentado e revertido — no vLLM o `response_format: json_object` liga
  guided decoding e a extração de 2 capítulos passou de minutos para +20 min; no
  Ollama o `format:"json"` provoca enchente de espaço em branco. Se voltar, usar
  backend xgrammar no vLLM e medir antes.
- Feito: **registro corrente no prompt** (nomes canônicos já conhecidos, do vault e
  das janelas anteriores da rodada), **grounding contra o texto**, **rejeição de
  termo relacional**, **dedup contra entidades existentes** e **verificação
  direcionada de entidades duvidosas** (só nomes ambíguos vão ao dedup-LLM).
- Feito: **dedup com contexto** (`dedupWithContext`, off por padrão) e **relação
  pendente** com promoção manual.

## Ideias — tempo de extração

- Feito: **paralelizar as janelas** (`extractionConcurrency`, só nuvem por
  padrão) e **modelo dedicado à extração** (`extractionModel`).
- **Pular dedup-LLM** quando há poucas entidades (já é barato, mas evita 1 call).
- **Cache por `doc_id` (BLAKE3)**: reingestão idêntica não re-extrai (o id já é
  content-addressed; falta ligar isso ao set de extraídos).
- **Incremental** (já implementado): só documentos novos são processados.
