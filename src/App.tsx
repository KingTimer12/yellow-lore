import { Match, Show, Switch, onMount } from "solid-js";
import { state, actions } from "./store";
import { themeVars } from "./theme";
import { initWindow } from "./window";
import { loadAppVersion } from "./version";
import { checkForUpdates } from "./update";
import TitleBar from "./components/TitleBar";
import UpdateBanner from "./components/UpdateBanner";
import Sidebar from "./components/Sidebar";
import ChatView from "./components/ChatView";
import KnowledgeView from "./components/KnowledgeView";
import CharactersView from "./components/CharactersView";
import PlacesView from "./components/PlacesView";
import AbilitiesView from "./components/AbilitiesView";
import SettingsView from "./components/SettingsView";
import EditDrawer from "./components/EditDrawer";
import CreateVaultModal from "./components/CreateVaultModal";
import InformationModal from "./components/InformationModal";
import CitationModal from "./components/CitationModal";
import Dialog from "./components/Dialog";
import MentionPopover from "./components/MentionPopover";
import SourcePopover from "./components/SourcePopover";
import Ornament from "./components/Ornament";

/// Shown when there is no active vault (fresh install / all vaults deleted).
/// No sidebar — just create a vault, with a subtle link into Settings.
function NoVault() {
  return (
    <div class="app-pane flex-1 flex flex-col items-center justify-center gap-5 px-8 anim-view">
      <div class="w-15 h-15 rounded-14px bg-accent flex items-center justify-center shadow-md">
        <span class="font-display text-accent-fg text-26px font-700 leading-none pt-1">Y</span>
      </div>
      <div class="text-center max-w-400px flex flex-col gap-2.5">
        <div class="font-serif text-26px font-600">Um codex vazio</div>
        <Ornament class="max-w-64px mx-auto my-1" />
        <div class="font-reading text-15px text-fg-muted leading-[1.55]">
          Cada vault é um mundo isolado — uma obra, uma campanha, um universo. Crie o primeiro para reunir seus documentos e começar a interrogá-los.
        </div>
      </div>
      <button
        onClick={() => actions.openVaultModal()}
        class="px-6 py-3 rounded-8px bg-accent text-accent-fg text-13.5px font-bold cursor-pointer border-none transition-transform active:scale-95"
      >
        + Criar vault
      </button>
      <div
        onClick={() => actions.setView("settings")}
        class="text-12px text-fg-muted cursor-pointer transition-colors duration-150 hover:text-fg"
      >
        Configurações
      </div>
    </div>
  );
}

/// Full-screen Settings with a back link — used when there is no vault yet
/// (so there is no sidebar to navigate from).
function SettingsStandalone() {
  return (
    <div class="app-pane flex-1 flex flex-col min-w-0">
      <div class="px-8 pt-5 flex-none">
        <div
          onClick={() => actions.setView("chat")}
          class="inline-flex items-center gap-1.5 text-12.5px text-fg-muted cursor-pointer transition-colors duration-150 hover:text-fg"
        >
          ← Voltar
        </div>
      </div>
      <SettingsView />
    </div>
  );
}

export default function App() {
  onMount(() => { void actions.init(); void initWindow(); void loadAppVersion(); void checkForUpdates(); });
  const hasVault = () => !!state.activeVaultId;
  return (
    <div
      class="win-root flex w-100vw h-100vh box-border"
      style={themeVars(state.theme)}
    >
      <div class="app-shell flex flex-col flex-1 min-w-0 bg-bg text-fg font-sans overflow-hidden">
        <TitleBar />
        <UpdateBanner />
        <div class="flex flex-1 min-h-0 overflow-hidden">
          <Show
            when={hasVault()}
            fallback={
              <Show when={state.view === "settings"} fallback={<NoVault />}>
                <SettingsStandalone />
              </Show>
            }
          >
            <Sidebar />
            <div class="app-pane flex-1 flex flex-col min-w-0">
              <Switch>
                <Match when={state.view === "chat"}>
                  <ChatView />
                </Match>
                <Match when={state.view === "knowledge"}>
                  <KnowledgeView />
                </Match>
                <Match when={state.view === "characters"}>
                  <CharactersView />
                </Match>
                <Match when={state.view === "places"}>
                  <PlacesView />
                </Match>
                <Match when={state.view === "abilities"}>
                  <AbilitiesView />
                </Match>
                <Match when={state.view === "settings"}>
                  <SettingsView />
                </Match>
              </Switch>
              <EditDrawer />
            </div>
          </Show>
        </div>
        <CreateVaultModal />
        <InformationModal />
        <CitationModal />
        <Dialog />
        <MentionPopover />
        <SourcePopover />
      </div>
    </div>
  );
}
