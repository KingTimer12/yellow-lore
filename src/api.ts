import { invoke as tauriInvoke, Channel } from "@tauri-apps/api/core";
import type { Character, Doc, Message, Place, Relation, Session, Settings, Source, Vault } from "./store";

export type StoredMessage = { id: string; role: "user" | "assistant"; text: string; thinking: string; sources: Source[] };

// True only inside the Tauri webview. In a plain browser (`bun run dev`) we
// fall back to mock data so the UI still works for design/preview.
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type Answer = { text: string; sources: { doc: string; quote: string; text: string }[] };
export type Entities = { characters: Character[]; places: Place[]; relations: Relation[] };

// Streaming events emitted by the `ask_stream` command.
export type StreamEvent =
  | { type: "token"; value: string }
  | { type: "done"; sources: { doc: string; quote: string; text: string }[] }
  | { type: "error"; message: string };

export const api = {
  // config
  getConfig: () => tauriInvoke<Settings>("get_config"),
  saveConfig: (config: Settings) => tauriInvoke<void>("save_config", { config }),

  // vaults
  listVaults: () => tauriInvoke<Vault[]>("list_vaults"),
  getActiveVault: () => tauriInvoke<string | null>("get_active_vault"),
  setActiveVault: (id: string) => tauriInvoke<void>("set_active_vault", { id }),
  createVault: (name: string) => tauriInvoke<Vault>("create_vault", { name }),
  renameVault: (id: string, name: string) => tauriInvoke<void>("rename_vault", { id, name }),
  deleteVault: (id: string) => tauriInvoke<void>("delete_vault", { id }),

  // documents
  listDocuments: () => tauriInvoke<Doc[]>("list_documents"),
  ingestDocument: (name: string, content: string) =>
    tauriInvoke<Doc>("ingest_document", { name, content }),
  // Binary formats (PDF/DOCX): text is extracted in Rust from base64 bytes.
  ingestBinary: (name: string, data: string) =>
    tauriInvoke<Doc>("ingest_binary", { name, data }),
  removeDocument: (id: string) => tauriInvoke<void>("remove_document", { id }),
  // index freshness vs current embedding model + reindex
  indexInfo: () => tauriInvoke<{ indexed: string; current: string; stale: boolean }>("index_info"),
  reindex: () => tauriInvoke<number>("reindex"),

  // chat — history carries the whole conversation so the LLM keeps context
  ask: (question: string, history: { role: string; text: string }[]) =>
    tauriInvoke<Answer>("ask", { question, history }),
  // streaming chat — `onEvent` fires per token, then a final done/error event
  askStream: (
    question: string,
    history: { role: string; text: string }[],
    onEvent: (e: StreamEvent) => void,
  ) => {
    const channel = new Channel<StreamEvent>();
    channel.onmessage = onEvent;
    return tauriInvoke<void>("ask_stream", { question, history, onEvent: channel });
  },
  // stop an in-flight streaming answer
  cancelGeneration: () => tauriInvoke<void>("cancel_generation"),

  // chat sessions
  listSessions: () => tauriInvoke<Session[]>("list_sessions"),
  createSession: (title: string) => tauriInvoke<Session>("create_session", { title }),
  renameSession: (id: string, title: string) => tauriInvoke<void>("rename_session", { id, title }),
  deleteSession: (id: string) => tauriInvoke<void>("delete_session", { id }),
  sessionMessages: (id: string) => tauriInvoke<StoredMessage[]>("session_messages", { id }),
  generateSessionTitle: (id: string, question: string, answer: string) =>
    tauriInvoke<string>("generate_session_title", { id, question, answer }),
  addMessage: (session: string, role: string, text: string, thinking: string, sources: unknown) =>
    tauriInvoke<void>("add_message", { session, role, text, thinking, sources }),

  // entities
  getEntities: () => tauriInvoke<Entities>("get_entities"),
  // Incremental by default; `force` re-scans every document (still preserving
  // edited/added entities).
  extractEntities: (force = false) => tauriInvoke<Entities>("extract_entities", { force }),
  addCharacter: (character: Character) => tauriInvoke<void>("add_character", { character }),
  addPlace: (place: Place) => tauriInvoke<void>("add_place", { place }),
  updateCharacter: (character: Character) => tauriInvoke<void>("update_character", { character }),
  updatePlace: (place: Place) => tauriInvoke<void>("update_place", { place }),
  addRelation: (relation: Relation) => tauriInvoke<void>("add_relation", { relation }),
  removeRelation: (relation: Relation) => tauriInvoke<void>("remove_relation", { relation }),
};

// Mock RAG used only in browser preview (no Rust backend).
export function mockAnswer(question: string): Message {
  const q = question.toLowerCase();
  if (q.includes("kaelen"))
    return { role: "assistant", text: "Kaelen Thorne é um mercenário contratado para escoltar Elandra Voss através da Floresta de Vharren. Inicialmente motivado apenas pelo pagamento, desenvolve lealdade genuína a ela ao longo da jornada.", sources: [{ doc: "Crônicas de Vharren - Cap. 1-5.pdf", quote: '"Kaelen aceitou o contrato…"', text: "Kaelen aceitou o contrato sem saber que a exilada guardava mais segredos do que moedas para pagá-lo. A jornada pela Floresta de Vharren mudaria isso." }] };
  if (q.includes("elandra"))
    return { role: "assistant", text: "Elandra Voss é a protagonista: ex-arquivista da corte, exilada de Nym após ser acusada de traição. Busca provar sua inocência com a ajuda de Kaelen Thorne.", sources: [{ doc: "Crônicas de Vharren - Cap. 1-5.pdf", quote: '"Elandra jamais esqueceria…"', text: "Elandra jamais esqueceria o dia em que a guarda a arrastou da Biblioteca Real, acusada de uma traição que não cometeu." }] };
  if (q.includes("nym") || q.includes("conselho"))
    return { role: "assistant", text: "Nym é a capital do reino, sede do Conselho e da Biblioteca Real de onde Elandra foi expulsa.", sources: [{ doc: "Notas de worldbuilding.txt", quote: '"Nym se ergue sobre sete pontes…"', text: "Nym se ergue sobre sete pontes, cada uma vigiada por um dos sete conselheiros. É a sede do Conselho e da Biblioteca Real." }] };
  return { role: "assistant", text: "(preview no navegador) Não encontrei um trecho direto sobre isso. Rode via `bun run tauri dev` para usar o RAG real. Tente Elandra, Kaelen ou Nym.", sources: [] };
}
