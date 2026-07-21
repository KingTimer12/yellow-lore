import { Match, Switch, onMount } from "solid-js";
import { state, actions } from "./store";
import { themeVars } from "./theme";
import Sidebar from "./components/Sidebar";
import ChatView from "./components/ChatView";
import KnowledgeView from "./components/KnowledgeView";
import CharactersView from "./components/CharactersView";
import PlacesView from "./components/PlacesView";
import SettingsView from "./components/SettingsView";
import EditDrawer from "./components/EditDrawer";

export default function App() {
  onMount(() => { void actions.init(); });
  return (
    <div
      class="flex w-100vw h-100vh bg-bg text-fg font-sans overflow-hidden"
      style={themeVars(state.theme)}
    >
      <Sidebar />
      <div class="flex-1 flex flex-col min-w-0 relative">
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
          <Match when={state.view === "settings"}>
            <SettingsView />
          </Match>
        </Switch>
        <EditDrawer />
      </div>
    </div>
  );
}
