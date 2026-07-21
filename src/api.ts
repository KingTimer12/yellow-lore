import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { Character, Doc, Message, Place, Relation, Settings, Vault } from "./store";

// True only inside the Tauri webview. In a plain browser (`bun run dev`) we
// fall back to mock data so the UI still works for design/preview.
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type Answer = { text: string; sources: { doc: string; quote: string }[] };
export type Entities = { characters: Character[]; places: Place[]; relations: Relation[] };

export const api = {
  // config
  getConfig: () => tauriInvoke<Settings>("get_config"),
  saveConfig: (config: Settings) => tauriInvoke<void>("save_config", { config }),

  // vaults
  listVaults: () => tauriInvoke<Vault[]>("list_vaults"),
  getActiveVault: () => tauriInvoke<string>("get_active_vault"),
  setActiveVault: (id: string) => tauriInvoke<void>("set_active_vault", { id }),
  createVault: (name: string) => tauriInvoke<Vault>("create_vault", { name }),
  renameVault: (id: string, name: string) => tauriInvoke<void>("rename_vault", { id, name }),
  deleteVault: (id: string) => tauriInvoke<void>("delete_vault", { id }),

  // documents
  listDocuments: () => tauriInvoke<Doc[]>("list_documents"),
  ingestDocument: (name: string, content: string) =>
    tauriInvoke<Doc>("ingest_document", { name, content }),
  removeDocument: (id: string) => tauriInvoke<void>("remove_document", { id }),

  // chat
  ask: (question: string) => tauriInvoke<Answer>("ask", { question }),

  // entities
  getEntities: () => tauriInvoke<Entities>("get_entities"),
  extractEntities: () => tauriInvoke<Entities>("extract_entities"),
  updateCharacter: (character: Character) => tauriInvoke<void>("update_character", { character }),
  updatePlace: (place: Place) => tauriInvoke<void>("update_place", { place }),
};

// Mock RAG used only in browser preview (no Rust backend).
export function mockAnswer(question: string): Message {
  const q = question.toLowerCase();
  if (q.includes("kaelen"))
    return { role: "assistant", text: "Kaelen Thorne é um mercenário contratado para escoltar Elandra Voss através da Floresta de Vharren. Inicialmente motivado apenas pelo pagamento, desenvolve lealdade genuína a ela ao longo da jornada.", sources: [{ doc: "Crônicas de Vharren - Cap. 1-5.pdf", quote: '"Kaelen aceitou o contrato sem saber que a exilada guardava mais segredos do que moedas para pagá-lo."' }] };
  if (q.includes("elandra"))
    return { role: "assistant", text: "Elandra Voss é a protagonista: ex-arquivista da corte, exilada de Nym após ser acusada de traição. Busca provar sua inocência com a ajuda de Kaelen Thorne.", sources: [{ doc: "Crônicas de Vharren - Cap. 1-5.pdf", quote: '"Elandra jamais esqueceria o dia em que a guarda a arrastou da Biblioteca Real..."' }] };
  if (q.includes("nym") || q.includes("conselho"))
    return { role: "assistant", text: "Nym é a capital do reino, sede do Conselho e da Biblioteca Real de onde Elandra foi expulsa.", sources: [{ doc: "Notas de worldbuilding.txt", quote: '"Nym se ergue sobre sete pontes, cada uma vigiada por um dos sete conselheiros."' }] };
  return { role: "assistant", text: "(preview no navegador) Não encontrei um trecho direto sobre isso. Rode via `bun run tauri dev` para usar o RAG real. Tente Elandra, Kaelen ou Nym.", sources: [] };
}
