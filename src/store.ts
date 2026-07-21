import { createStore, produce } from "solid-js/store";
import { api, isTauri, mockAnswer } from "./api";

// ---- Domain types ---------------------------------------------------------

export type View = "chat" | "knowledge" | "characters" | "places" | "settings";

export type Source = { doc: string; quote: string };

export type Message = {
  role: "user" | "assistant";
  text: string;
  sources?: Source[];
};

export type Doc = {
  id: string;
  name: string;
  type: string;
  pages: number;
  status: string;
  addedLabel: string;
};

export type Character = {
  id: string;
  name: string;
  role: string;
  summary: string;
  traits: string[];
  status: string;
  sourceDoc: string;
  sourceQuote: string;
};

export type Place = {
  id: string;
  name: string;
  type: string;
  summary: string;
  status: string;
  sourceDoc: string;
  sourceQuote: string;
};

export type Relation = { from: string; to: string; label: string };

export type Vault = { id: string; name: string; createdAt: string };

/// Mirrors the Rust `RagConfig` (camelCase). LLM and embedding are independent.
export type Settings = {
  llmProvider: string;
  llmModel: string;
  embeddingProvider: string;
  embeddingModel: string;
  openaiApiKey: string;
  openaiBaseUrl: string;
  ollamaEndpoint: string;
  systemPrompt: string;
  chunkSize: number;
  chunkOverlap: number;
  topK: number;
  showSources: boolean;
};

export type EditTarget = { kind: "character" | "place"; id: string } | null;

export type EditForm = {
  name: string;
  role: string;
  summary: string;
  traitsText: string;
  sourceDoc: string;
  sourceQuote: string;
};

export type State = {
  view: View;
  theme: "dark" | "light";
  vaults: Vault[];
  activeVaultId: string;
  extracting: boolean;
  chatInput: string;
  messages: Message[];
  pending: boolean;
  docsFilter: string;
  docs: Doc[];
  charactersTab: "grid" | "relations";
  characters: Character[];
  relations: Relation[];
  places: Place[];
  editing: EditTarget;
  editForm: EditForm | null;
  settings: Settings;
  savedToast: boolean;
};

export const DEFAULT_SETTINGS: Settings = {
  llmProvider: "ollama",
  llmModel: "llama3.1",
  embeddingProvider: "ollama",
  embeddingModel: "nomic-embed-text",
  openaiApiKey: "",
  openaiBaseUrl: "https://api.openai.com/v1",
  ollamaEndpoint: "http://localhost:11434",
  systemPrompt:
    "Você é o assistente do Yellow Lore. Responda SEMPRE com base nos trechos de conhecimento fornecidos no contexto. Se a resposta não estiver no contexto, diga que não encontrou nos documentos indexados.",
  chunkSize: 800,
  chunkOverlap: 120,
  topK: 5,
  showSources: true,
};

// ---- Seed data (browser preview only) -------------------------------------

const initial: State = {
  view: "chat",
  theme: "dark",
  vaults: [{ id: "local", name: "Yellow Lore", createdAt: "" }],
  activeVaultId: "local",
  extracting: false,
  chatInput: "",
  pending: false,
  messages: [
    { role: "assistant", text: "Olá! Sou seu assistente de conhecimento. Carregue documentos na aba Conhecimento e pergunte sobre eles — eu sempre busco na base antes de responder." },
  ],
  docsFilter: "",
  docs: [
    { id: "d1", name: "Crônicas de Vharren - Cap. 1-5.pdf", type: "PDF", pages: 82, status: "Indexado", addedLabel: "há 3 dias" },
    { id: "d2", name: "Notas de worldbuilding.txt", type: "TXT", pages: 14, status: "Indexado", addedLabel: "há 3 dias" },
    { id: "d3", name: "Diálogos - Ato II.docx", type: "DOCX", pages: 31, status: "Processando", addedLabel: "há 2 horas" },
  ],
  charactersTab: "grid",
  characters: [
    { id: "c1", name: "Elandra Voss", role: "Protagonista, exilada da corte", summary: "Ex-arquivista real acusada de traição, agora busca provar sua inocência.", traits: ["Estratégica", "Desconfiada", "Leal aos poucos"], status: "Extraído", sourceDoc: "Crônicas de Vharren - Cap. 1-5.pdf", sourceQuote: '"Elandra jamais esqueceria o dia em que a guarda a arrastou da Biblioteca Real..."' },
    { id: "c2", name: "Kaelen Thorne", role: "Mercenário, aliado de Elandra", summary: "Contratado para escoltar Elandra, desenvolve lealdade inesperada.", traits: ["Cínico", "Habilidoso com espadas", "Passado obscuro"], status: "Editado", sourceDoc: "Crônicas de Vharren - Cap. 1-5.pdf", sourceQuote: '"Kaelen aceitou o contrato sem saber que a exilada guardava mais segredos do que moedas para pagá-lo."' },
    { id: "c3", name: "Vozes do Conselho", role: "Antagonista coletivo", summary: "Conselho de nobres que orquestrou a queda de Elandra.", traits: ["Corrupto", "Poderoso"], status: "Extraído", sourceDoc: "Notas de worldbuilding.txt", sourceQuote: '"O Conselho de Nym decide em sete votos — e Elandra teve contra si os sete."' },
  ],
  relations: [
    { from: "Elandra Voss", to: "Kaelen Thorne", label: "aliado de" },
    { from: "Elandra Voss", to: "Vozes do Conselho", label: "perseguida por" },
  ],
  places: [
    { id: "p1", name: "Cidade de Nym", type: "Capital", summary: "Capital do reino, sede do Conselho e da Biblioteca Real.", status: "Extraído", sourceDoc: "Notas de worldbuilding.txt", sourceQuote: '"Nym se ergue sobre sete pontes, cada uma vigiada por um dos sete conselheiros."' },
    { id: "p2", name: "Floresta de Vharren", type: "Região selvagem", summary: "Floresta antiga onde Elandra se refugia após o exílio.", status: "Extraído", sourceDoc: "Crônicas de Vharren - Cap. 1-5.pdf", sourceQuote: '"A Floresta de Vharren não perdoa quem entra sem convite dos antigos."' },
  ],
  editing: null,
  editForm: null,
  settings: DEFAULT_SETTINGS,
  savedToast: false,
};

// ---- Store + actions ------------------------------------------------------

const [state, setState] = createStore<State>(structuredClone(initial));
export { state };

let toastTimer: ReturnType<typeof setTimeout> | undefined;

/// Reload the docs + extracted entities for the active vault (Tauri only).
async function loadActiveVault() {
  if (!isTauri) return;
  const [docs, entities] = await Promise.all([api.listDocuments(), api.getEntities()]);
  setState({
    docs,
    characters: entities.characters,
    places: entities.places,
    relations: entities.relations,
  });
}

export const actions = {
  /// Load config, vaults and the active vault's data from the Rust backend.
  async init() {
    if (!isTauri) return;
    try {
      const [cfg, vaults, activeVaultId] = await Promise.all([
        api.getConfig(),
        api.listVaults(),
        api.getActiveVault(),
      ]);
      setState({ settings: cfg, vaults, activeVaultId });
      await loadActiveVault();
    } catch (e) {
      console.error("init falhou", e);
    }
  },

  // --- Vaults ---
  async selectVault(id: string) {
    setState({ activeVaultId: id });
    if (isTauri) {
      await api.setActiveVault(id);
      await loadActiveVault();
    }
  },
  async createVault(name: string) {
    const clean = name.trim();
    if (!clean) return;
    if (isTauri) {
      const vault = await api.createVault(clean);
      setState(produce((s) => { s.vaults.push(vault); s.activeVaultId = vault.id; }));
      await loadActiveVault();
    } else {
      const vault: Vault = { id: crypto.randomUUID(), name: clean, createdAt: "" };
      setState(produce((s) => {
        s.vaults.push(vault);
        s.activeVaultId = vault.id;
        s.docs = []; s.characters = []; s.places = []; s.relations = [];
      }));
    }
  },
  async renameVault(id: string, name: string) {
    const clean = name.trim();
    if (!clean) return;
    if (isTauri) await api.renameVault(id, clean);
    setState("vaults", (v) => v.id === id, "name", clean);
  },
  async deleteVault(id: string) {
    if (state.vaults.length <= 1) return;
    if (isTauri) await api.deleteVault(id);
    const remaining = state.vaults.filter((v) => v.id !== id);
    setState("vaults", remaining);
    if (state.activeVaultId === id && remaining[0]) {
      await actions.selectVault(remaining[0].id);
    }
  },

  setView: (view: View) => setState({ view }),
  toggleTheme: () => setState("theme", (t) => (t === "dark" ? "light" : "dark")),

  setChatInput: (chatInput: string) => setState({ chatInput }),
  async sendMessage() {
    const text = state.chatInput.trim();
    if (!text || state.pending) return;
    setState(produce((s) => {
      s.messages.push({ role: "user", text });
      s.chatInput = "";
      s.pending = true;
    }));
    try {
      const reply: Message = isTauri
        ? { role: "assistant", ...(await api.ask(text)) }
        : mockAnswer(text);
      setState("messages", (m) => [...m, reply]);
    } catch (e) {
      setState("messages", (m) => [
        ...m,
        { role: "assistant", text: `Erro ao consultar: ${e}`, sources: [] },
      ]);
    } finally {
      setState("pending", false);
    }
  },

  setDocsFilter: (docsFilter: string) => setState({ docsFilter }),
  async ingestDocument(name: string, content: string) {
    if (isTauri) {
      const doc = await api.ingestDocument(name, content);
      setState("docs", (d) => [doc, ...d.filter((x) => x.id !== doc.id)]);
    } else {
      const doc: Doc = { id: crypto.randomUUID(), name, type: (name.split(".").pop() ?? "TXT").toUpperCase(), pages: 1, status: "Indexado", addedLabel: "agora" };
      setState("docs", (d) => [doc, ...d]);
    }
  },
  async removeDoc(id: string) {
    if (isTauri) await api.removeDocument(id).catch((e) => console.error(e));
    setState("docs", (docs) => docs.filter((d) => d.id !== id));
  },

  setCharactersTab: (charactersTab: "grid" | "relations") => setState({ charactersTab }),

  /// Run LLM extraction over the active vault's knowledge → characters/places/relations.
  async extractEntities() {
    if (state.extracting) return;
    if (!isTauri) return;
    setState("extracting", true);
    try {
      const entities = await api.extractEntities();
      setState({
        characters: entities.characters,
        places: entities.places,
        relations: entities.relations,
      });
    } catch (e) {
      console.error("extração falhou", e);
      alert(`Extração falhou: ${e}`);
    } finally {
      setState("extracting", false);
    }
  },

  openEdit: (kind: "character" | "place", id: string) => {
    const entity =
      kind === "character"
        ? state.characters.find((c) => c.id === id)
        : state.places.find((p) => p.id === id);
    if (!entity) return;
    setState({
      editing: { kind, id },
      editForm: {
        name: entity.name,
        role: kind === "character" ? (entity as Character).role : (entity as Place).type,
        summary: entity.summary,
        traitsText: kind === "character" ? (entity as Character).traits.join(", ") : "",
        sourceDoc: entity.sourceDoc,
        sourceQuote: entity.sourceQuote,
      },
    });
  },
  closeEdit: () => setState({ editing: null, editForm: null }),
  setEditField: (field: keyof EditForm, value: string) =>
    setState("editForm", (f) => (f ? { ...f, [field]: value } : f)),
  async saveEdit() {
    const editing = state.editing;
    const form = state.editForm;
    if (!editing || !form) return;
    if (editing.kind === "character") {
      setState("characters", (c) => c.id === editing.id, produce((c: Character) => {
        c.name = form.name;
        c.role = form.role;
        c.summary = form.summary;
        c.traits = form.traitsText.split(",").map((t) => t.trim()).filter(Boolean);
        c.status = "Editado";
      }));
      if (isTauri) {
        const c = state.characters.find((x) => x.id === editing.id);
        if (c) await api.updateCharacter({ ...c }).catch((e) => console.error(e));
      }
    } else {
      setState("places", (p) => p.id === editing.id, produce((p: Place) => {
        p.name = form.name;
        p.type = form.role;
        p.summary = form.summary;
        p.status = "Editado";
      }));
      if (isTauri) {
        const p = state.places.find((x) => x.id === editing.id);
        if (p) await api.updatePlace({ ...p }).catch((e) => console.error(e));
      }
    }
    setState({ editing: null, editForm: null });
  },

  setSetting: <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setState("settings", key, value),
  async saveSettings() {
    if (isTauri) {
      try {
        await api.saveConfig({ ...state.settings });
      } catch (e) {
        console.error("saveConfig falhou", e);
      }
    }
    setState({ savedToast: true });
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => setState({ savedToast: false }), 2200);
  },
};
