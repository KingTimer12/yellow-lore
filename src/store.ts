import { createStore, produce } from "solid-js/store";
import { api, isTauri, mockAnswer } from "./api";

// ---- Domain types ---------------------------------------------------------

export type View = "chat" | "knowledge" | "characters" | "places" | "settings";

export type Source = { doc: string; quote: string; text: string };

export type Message = {
  role: "user" | "assistant";
  text: string;
  /// Reasoning captured from a `<think>…</think>` block, shown separately.
  thinking?: string;
  sources?: Source[];
};

/// Split a raw model reply into its `<think>` reasoning and the final answer.
/// Handles the still-open case during streaming (no closing tag yet).
export function splitThink(raw: string): { thinking: string; text: string } {
  const open = raw.indexOf("<think>");
  if (open === -1) return { thinking: "", text: raw };
  const pre = raw.slice(0, open);
  const after = raw.slice(open + "<think>".length);
  const close = after.indexOf("</think>");
  if (close === -1) return { thinking: after, text: pre };
  return { thinking: after.slice(0, close), text: pre + after.slice(close + "</think>".length) };
}

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

export type Session = { id: string; title: string; createdAt: string; updatedAt: string };

/// Mirrors the Rust `RagConfig` (camelCase). LLM and embedding are independent.
export type Settings = {
  llmProvider: string;
  llmModel: string;
  embeddingProvider: string;
  embeddingModel: string;
  openaiApiKey: string;
  openaiBaseUrl: string;
  ollamaEndpoint: string;
  ollamaNumCtx: number;
  vllmBaseUrl: string;
  vllmApiKey: string;
  systemPrompt: string;
  chunkSize: number;
  chunkOverlap: number;
  topK: number;
  temperature: number;
  showSources: boolean;
  dedupEntities: boolean;
  /// Empty = reuse the chat model for extraction (no second model to load).
  extractionModel: string;
  /// Concurrent extraction windows. 1 = sequential (safe for a single local GPU).
  extractionConcurrency: number;
  /// Extra LLM pass that re-orders retrieved chunks by relevance before answering.
  rerank: boolean;
  /// Corrective RAG: draft → auto-avaliação → uma re-busca mais ampla se preciso.
  corrective: boolean;
  /// Deixar a resposta final "pensar" (bloco de raciocínio). Off por padrão.
  showThinking: boolean;
};

// `creating` marks a brand-new entity (manual add) vs editing an existing one.
export type EditTarget = { kind: "character" | "place"; id: string; creating?: boolean } | null;

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
  /// Saved conversations for the active vault. `currentSessionId` is "" for a
  /// fresh, not-yet-saved chat (the app opens on this blank state).
  sessions: Session[];
  currentSessionId: string;
  pending: boolean;
  docsFilter: string;
  docs: Doc[];
  charactersTab: "grid" | "graph";
  characters: Character[];
  relations: Relation[];
  places: Place[];
  editing: EditTarget;
  editForm: EditForm | null;
  settings: Settings;
  savedToast: boolean;
  vaultModalOpen: boolean;
  informationModalOpen: boolean;
  /// True when the vault was embedded with a different model than the current
  /// one — offer a reindex. `reindexing` guards the in-flight re-embed.
  indexStale: boolean;
  reindexing: boolean;
  /// Citation modal: the document and its retrieved passages, or null.
  citation: { doc: string; passages: { quote: string; text: string }[] } | null;
  /// Free-text filter for the Characters grid (name / role / trait).
  charactersFilter: string;
  /// Hover-card for an entity mentioned in a chat reply (null = hidden). `x`/`y`
  /// are viewport coords of the mention; the popover anchors there.
  mention: {
    name: string;
    role: string;
    summary: string;
    traits: string[];
    kind: "character" | "place";
    x: number;
    y: number;
  } | null;
  /// Centralized dialog (replaces native alert/confirm). `mode` picks the shape:
  /// "alert" = single OK; "confirm" = confirm/cancel. The confirm callback lives
  /// outside the store (see `pendingConfirm`) — stores hold only serializable data.
  dialog: {
    mode: "alert" | "confirm" | "prompt";
    title: string;
    message: string;
    confirmLabel: string;
    cancelLabel: string;
    danger: boolean;
    /// Prompt only: current text-input value + placeholder.
    value: string;
    placeholder: string;
  } | null;
};

export const DEFAULT_SETTINGS: Settings = {
  llmProvider: "ollama",
  llmModel: "llama3.1",
  embeddingProvider: "ollama",
  embeddingModel: "nomic-embed-text",
  openaiApiKey: "",
  openaiBaseUrl: "https://api.openai.com/v1",
  ollamaEndpoint: "http://localhost:11434",
  ollamaNumCtx: 8192,
  vllmBaseUrl: "http://localhost:8000/v1",
  vllmApiKey: "",
  systemPrompt:
    "Você é o assistente do Yellow Lore. Responda SEMPRE com base nos trechos de conhecimento fornecidos no contexto. Se a resposta não estiver no contexto, diga que não encontrou nos documentos indexados.",
  chunkSize: 800,
  chunkOverlap: 120,
  topK: 5,
  temperature: 0.2,
  showSources: true,
  dedupEntities: true,
  extractionModel: "",
  extractionConcurrency: 1,
  rerank: false,
  corrective: false,
  showThinking: false,
};

// ---- Seed data (browser preview only) -------------------------------------

const initial: State = {
  view: "chat",
  theme: "dark",
  // Vaults start empty (Obsidian-style) — the user creates the first vault.
  vaults: [],
  activeVaultId: "",
  extracting: false,
  chatInput: "",
  pending: false,
  // Chat starts empty, like a fresh AI conversation.
  messages: [],
  sessions: [],
  currentSessionId: "",
  docsFilter: "",
  docs: [],
  charactersTab: "grid",
  characters: [],
  relations: [],
  places: [],
  editing: null,
  editForm: null,
  settings: DEFAULT_SETTINGS,
  savedToast: false,
  vaultModalOpen: false,
  informationModalOpen: false,
  indexStale: false,
  reindexing: false,
  citation: null,
  charactersFilter: "",
  mention: null,
  dialog: null,
};

// ---- Store + actions ------------------------------------------------------

const [state, setState] = createStore<State>(structuredClone(initial));

// Pending callback for the centralized dialog. Kept out of the store so only
// serializable data lives there; resolved by `resolveDialog`. Receives the
// prompt's input value (ignored by alert/confirm).
let pendingConfirm: ((value: string) => void) | null = null;
export { state };

let toastTimer: ReturnType<typeof setTimeout> | undefined;

/// Reload the docs + extracted entities for the active vault (Tauri only).
async function loadActiveVault() {
  if (!isTauri) return;
  const [docs, entities, index, sessions] = await Promise.all([
    api.listDocuments(),
    api.getEntities(),
    api.indexInfo().catch(() => ({ stale: false })),
    api.listSessions().catch(() => [] as Session[]),
  ]);
  setState({
    docs,
    characters: entities.characters,
    places: entities.places,
    relations: entities.relations,
    indexStale: index.stale,
    sessions,
    // Open on a blank new chat — "what do you want to do?".
    messages: [],
    currentSessionId: "",
  });
}

/// Move a session to the top of the list (most recent activity first).
function bumpSession(id: string) {
  setState("sessions", (list) => {
    const found = list.find((s) => s.id === id);
    if (!found) return list;
    return [found, ...list.filter((s) => s.id !== id)];
  });
}

async function refreshIndexInfo() {
  if (!isTauri) return;
  try {
    const info = await api.indexInfo();
    setState({ indexStale: info.stale });
  } catch {
    /* no active vault yet */
  }
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
      setState({ settings: cfg, vaults, activeVaultId: activeVaultId ?? "" });
      if (activeVaultId) await loadActiveVault();
    } catch (e) {
      console.error("init falhou", e);
    }
  },

  // --- Vaults ---
  async selectVault(id: string) {
    // Switching vault = new knowledge base → fresh conversation.
    setState({ activeVaultId: id, messages: [] });
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
      setState(produce((s) => { s.vaults.push(vault); s.activeVaultId = vault.id; s.messages = []; }));
      await loadActiveVault();
    } else {
      const vault: Vault = { id: crypto.randomUUID(), name: clean, createdAt: "" };
      setState(produce((s) => {
        s.vaults.push(vault);
        s.activeVaultId = vault.id;
        s.messages = [];
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
    if (isTauri) await api.deleteVault(id);
    const remaining = state.vaults.filter((v) => v.id !== id);
    setState("vaults", remaining);
    if (state.activeVaultId === id) {
      if (remaining[0]) {
        await actions.selectVault(remaining[0].id);
      } else {
        // No vaults left → back to the empty state.
        setState({ activeVaultId: "", messages: [], docs: [], characters: [], places: [], relations: [], sessions: [], currentSessionId: "" });
      }
    }
  },

  openInformationModal: () => setState({ informationModalOpen: true }),
  closeInformationModal: () => setState({ informationModalOpen: false }),
  openVaultModal: () => setState({ vaultModalOpen: true }),
  closeVaultModal: () => setState({ vaultModalOpen: false }),
  /// Confirm the create-vault modal: persist the chosen provider settings, then
  /// create + activate the vault.
  async confirmCreateVault(name: string) {
    const clean = name.trim();
    if (!clean) return;
    if (isTauri) {
      try {
        await api.saveConfig({ ...state.settings });
      } catch (e) {
        console.error("saveConfig falhou", e);
      }
    }
    await actions.createVault(clean);
    setState({ vaultModalOpen: false, view: "chat" });
  },

  openCitation: (doc: string, passages: { quote: string; text: string }[]) =>
    setState({ citation: { doc, passages } }),
  closeCitation: () => setState({ citation: null }),

  setView: (view: View) => setState({ view }),
  toggleTheme: () => setState("theme", (t) => (t === "dark" ? "light" : "dark")),

  // --- Chat sessions ---
  /// Start a fresh, unsaved conversation (persisted on the first message).
  newChat: () => setState({ messages: [], currentSessionId: "" }),
  async openSession(id: string) {
    if (!isTauri) { setState({ currentSessionId: id }); return; }
    setState({ currentSessionId: id });
    try {
      const msgs = await api.sessionMessages(id);
      setState("messages", msgs.map((m) => ({
        role: m.role,
        text: m.text,
        thinking: m.thinking || undefined,
        sources: m.sources,
      })));
    } catch (e) {
      console.error("sessão falhou", e);
    }
  },
  async deleteSession(id: string) {
    if (isTauri) await api.deleteSession(id).catch((e) => console.error(e));
    setState("sessions", (s) => s.filter((x) => x.id !== id));
    if (state.currentSessionId === id) setState({ messages: [], currentSessionId: "" });
  },
  async renameSession(id: string, title: string) {
    const t = title.trim();
    if (!t) return;
    if (isTauri) await api.renameSession(id, t).catch((e) => console.error(e));
    setState("sessions", (s) => s.id === id, "title", t);
  },

  setChatInput: (chatInput: string) => setState({ chatInput }),
  /// Stop an in-flight answer. Backend returns the partial text and fires the
  /// `done` event, which clears `pending`.
  stopGeneration() {
    if (!state.pending) return;
    if (isTauri) api.cancelGeneration().catch((e) => console.error(e));
  },
  async sendMessage() {
    const text = state.chatInput.trim();
    if (!text || state.pending) return;
    // Snapshot the conversation so far as memory for the LLM (excludes the
    // thinking blocks — only the final answers are replayed as context). Skip any
    // malformed or empty turn: a generation that was cancelled/truncated leaves an
    // assistant message with empty text (and possibly no role), which would both
    // pollute context and crash the command args (`missing field role`).
    const history = state.messages
      .filter((m) => (m.role === "user" || m.role === "assistant") && m.text.trim())
      .map((m) => ({ role: m.role, text: m.text }));

    // A blank chat becomes a new saved session on its first message.
    let sessionId = state.currentSessionId;
    const isNewSession = isTauri && !sessionId;
    if (isTauri && !sessionId) {
      const title = text.length > 48 ? text.slice(0, 48) + "…" : text;
      try {
        const s = await api.createSession(title);
        sessionId = s.id;
        setState(produce((st) => { st.currentSessionId = s.id; st.sessions.unshift(s); }));
      } catch (e) {
        console.error("criar sessão falhou", e);
      }
    }

    setState(produce((s) => {
      s.messages.push({ role: "user", text });
      s.messages.push({ role: "assistant", text: "", thinking: "" });
      s.chatInput = "";
      s.pending = true;
    }));
    const idx = state.messages.length - 1; // the assistant placeholder

    if (!isTauri) {
      const reply = mockAnswer(text);
      setState("messages", idx, (m) => ({ ...m, text: reply.text, sources: reply.sources }));
      setState("pending", false);
      return;
    }

    // Persist the user turn immediately (survives a failed generation).
    if (sessionId) api.addMessage(sessionId, "user", text, "", []).catch((e) => console.error(e));

    let raw = "";
    await new Promise<void>((resolve) => {
      const finish = () => { setState("pending", false); resolve(); };
      api
        .askStream(text, history, (e) => {
          if (e.type === "token") {
            raw += e.value;
            const { thinking, text: body } = splitThink(raw);
            setState("messages", idx, (m) => ({ ...m, thinking, text: body }));
          } else if (e.type === "done") {
            const { thinking, text: body } = splitThink(raw);
            setState("messages", idx, (m) => ({ ...m, sources: e.sources }));
            if (sessionId) {
              api.addMessage(sessionId, "assistant", body, thinking, e.sources).catch((er) => console.error(er));
              bumpSession(sessionId);
              // Summarize the first exchange into a concise title (like ChatGPT).
              if (isNewSession && body.trim()) {
                api.generateSessionTitle(sessionId, text, body)
                  .then((title) => {
                    if (title.trim()) setState("sessions", (s) => s.id === sessionId, "title", title.trim());
                  })
                  .catch((er) => console.error(er));
              }
            }
            finish();
          } else if (e.type === "error") {
            const msg = `Erro ao consultar: ${e.message}`;
            setState("messages", idx, (m) => ({ ...m, text: msg }));
            if (sessionId) api.addMessage(sessionId, "assistant", msg, "", []).catch((er) => console.error(er));
            finish();
          }
        })
        .catch((err) => {
          setState("messages", idx, (m) => ({ ...m, text: `Erro ao consultar: ${err}` }));
          finish();
        });
    });
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
  /// Ingest a binary document (PDF/DOCX) — `data` is base64 of the file bytes.
  async ingestBinaryDocument(name: string, data: string) {
    if (isTauri) {
      const doc = await api.ingestBinary(name, data);
      setState("docs", (d) => [doc, ...d.filter((x) => x.id !== doc.id)]);
    } else {
      const doc: Doc = { id: crypto.randomUUID(), name, type: (name.split(".").pop() ?? "DOC").toUpperCase(), pages: 1, status: "Indexado", addedLabel: "agora" };
      setState("docs", (d) => [doc, ...d]);
    }
  },
  async removeDoc(id: string) {
    if (isTauri) await api.removeDocument(id).catch((e) => console.error(e));
    setState("docs", (docs) => docs.filter((d) => d.id !== id));
  },

  /// Re-embed the whole vault with the current embedding model.
  async reindex() {
    if (!isTauri || state.reindexing) return;
    setState("reindexing", true);
    try {
      await api.reindex();
      setState("indexStale", false);
    } catch (e) {
      actions.notify(`${e}`, "Reindexação falhou");
    } finally {
      setState("reindexing", false);
    }
  },

  setCharactersTab: (charactersTab: "grid" | "graph") => setState({ charactersTab }),

  /// Run LLM extraction over the active vault's knowledge → characters/places/relations.
  /// Incremental by default (only new documents); `force` re-scans everything.
  /// Entities the user edited or added are never overwritten either way.
  async extractEntities(force = false) {
    if (state.extracting) return;
    if (!isTauri) return;
    setState("extracting", true);
    try {
      const entities = await api.extractEntities(force);
      setState({
        characters: entities.characters,
        places: entities.places,
        relations: entities.relations,
      });
    } catch (e) {
      console.error("extração falhou", e);
      actions.notify(`${e}`, "Extração falhou");
    } finally {
      setState("extracting", false);
    }
  },

  /// Open the drawer to create a new entity by hand (status "Adicionado").
  openCreate: (kind: "character" | "place") => {
    setState({
      editing: { kind, id: crypto.randomUUID(), creating: true },
      editForm: { name: "", role: "", summary: "", traitsText: "", sourceDoc: "", sourceQuote: "" },
    });
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
    if (!form.name.trim()) return; // a name is the one required field
    const creating = !!editing.creating;
    // Manual adds are "Adicionado"; edits become "Editado". Both are protected
    // from being overwritten by future extraction runs.
    const status = creating ? "Adicionado" : "Editado";
    if (editing.kind === "character") {
      if (creating) {
        const c: Character = {
          id: editing.id,
          name: form.name,
          role: form.role,
          summary: form.summary,
          traits: form.traitsText.split(",").map((t) => t.trim()).filter(Boolean),
          status,
          sourceDoc: "",
          sourceQuote: "",
        };
        setState("characters", (list) => [c, ...list]);
        if (isTauri) await api.addCharacter({ ...c }).catch((e) => console.error(e));
      } else {
        setState("characters", (c) => c.id === editing.id, produce((c: Character) => {
          c.name = form.name;
          c.role = form.role;
          c.summary = form.summary;
          c.traits = form.traitsText.split(",").map((t) => t.trim()).filter(Boolean);
          c.status = status;
        }));
        if (isTauri) {
          const c = state.characters.find((x) => x.id === editing.id);
          if (c) await api.updateCharacter({ ...c }).catch((e) => console.error(e));
        }
      }
    } else {
      if (creating) {
        const p: Place = {
          id: editing.id,
          name: form.name,
          type: form.role,
          summary: form.summary,
          status,
          sourceDoc: "",
          sourceQuote: "",
        };
        setState("places", (list) => [p, ...list]);
        if (isTauri) await api.addPlace({ ...p }).catch((e) => console.error(e));
      } else {
        setState("places", (p) => p.id === editing.id, produce((p: Place) => {
          p.name = form.name;
          p.type = form.role;
          p.summary = form.summary;
          p.status = status;
        }));
        if (isTauri) {
          const p = state.places.find((x) => x.id === editing.id);
          if (p) await api.updatePlace({ ...p }).catch((e) => console.error(e));
        }
      }
    }
    setState({ editing: null, editForm: null });
  },

  /// Manually add a graph edge. Skips duplicates and no-op self-links; optimistic.
  async addRelation(from: string, to: string, label: string) {
    from = from.trim();
    to = to.trim();
    label = label.trim();
    if (!from || !to || from.toLowerCase() === to.toLowerCase()) return;
    const dup = state.relations.some(
      (r) =>
        r.from.toLowerCase() === from.toLowerCase() &&
        r.to.toLowerCase() === to.toLowerCase() &&
        r.label.toLowerCase() === label.toLowerCase(),
    );
    if (dup) return;
    const rel: Relation = { from, to, label };
    setState("relations", (list) => [...list, rel]);
    if (isTauri) await api.addRelation({ ...rel }).catch((e) => console.error(e));
  },

  /// Remove a graph edge by its exact (from, to, label) triple; optimistic.
  async removeRelation(rel: Relation) {
    setState("relations", (list) =>
      list.filter(
        (r) =>
          !(
            r.from.toLowerCase() === rel.from.toLowerCase() &&
            r.to.toLowerCase() === rel.to.toLowerCase() &&
            r.label.toLowerCase() === rel.label.toLowerCase()
          ),
      ),
    );
    if (isTauri) await api.removeRelation({ ...rel }).catch((e) => console.error(e));
  },

  setCharactersFilter: (v: string) => setState("charactersFilter", v),

  showMention: (m: NonNullable<State["mention"]>) => setState("mention", m),
  hideMention: () => setState("mention", null),

  /// Show an informational dialog (replaces native `alert`).
  notify(message: string, title = "Aviso") {
    pendingConfirm = null;
    setState("dialog", {
      mode: "alert",
      title,
      message,
      confirmLabel: "Entendi",
      cancelLabel: "",
      danger: false,
      value: "",
      placeholder: "",
    });
  },

  /// Ask for a single line of text (replaces native `prompt`). `onSubmit` runs
  /// with the trimmed value only if it's non-empty and the user confirms.
  askPrompt(opts: {
    title: string;
    message?: string;
    defaultValue?: string;
    placeholder?: string;
    confirmLabel?: string;
    onSubmit: (value: string) => void;
  }) {
    pendingConfirm = (v: string) => {
      const t = v.trim();
      if (t) opts.onSubmit(t);
    };
    setState("dialog", {
      mode: "prompt",
      title: opts.title,
      message: opts.message ?? "",
      confirmLabel: opts.confirmLabel ?? "Salvar",
      cancelLabel: "Cancelar",
      danger: false,
      value: opts.defaultValue ?? "",
      placeholder: opts.placeholder ?? "",
    });
  },

  setDialogValue: (v: string) => setState("dialog", (d) => (d ? { ...d, value: v } : d)),

  /// Ask for confirmation (replaces native `confirm`). `onConfirm` runs only if
  /// the user confirms.
  askConfirm(opts: {
    title: string;
    message: string;
    confirmLabel?: string;
    danger?: boolean;
    onConfirm: () => void;
  }) {
    pendingConfirm = () => opts.onConfirm();
    setState("dialog", {
      mode: "confirm",
      title: opts.title,
      message: opts.message,
      confirmLabel: opts.confirmLabel ?? "Confirmar",
      cancelLabel: "Cancelar",
      danger: opts.danger ?? false,
      value: "",
      placeholder: "",
    });
  },

  /// Resolve the open dialog. Runs the pending callback when `ok`, passing the
  /// prompt input value (empty for alert/confirm).
  resolveDialog(ok: boolean) {
    const cb = pendingConfirm;
    const val = state.dialog?.value ?? "";
    pendingConfirm = null;
    setState("dialog", null);
    if (ok && cb) cb(val);
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
    await refreshIndexInfo();
    setState({ savedToast: true });
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => setState({ savedToast: false }), 2200);
  },
};
