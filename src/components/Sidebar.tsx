import { For, Show } from "solid-js";
import { state, actions, type View } from "../store";
import VaultSwitcher from "./VaultSwitcher";
import Ornament from "./Ornament";
import logo from "../assets/logo.svg";

function openSession(id: string) {
  actions.openSession(id);
  actions.setView("chat");
}
function newChat() {
  actions.newChat();
  actions.setView("chat");
}
function renameSession(e: MouseEvent, id: string, current: string) {
  e.stopPropagation();
  actions.askPrompt({
    title: "Renomear conversa",
    defaultValue: current,
    placeholder: "Título da conversa",
    onSubmit: (t) => actions.renameSession(id, t),
  });
}
function removeSession(e: MouseEvent, id: string, title: string) {
  e.stopPropagation();
  actions.askConfirm({
    title: "Excluir conversa?",
    message: `Excluir a conversa "${title}"? O histórico dela será perdido.`,
    confirmLabel: "Excluir",
    danger: true,
    onConfirm: () => void actions.deleteSession(id),
  });
}

type NavDef = { id: View; label: string; icon: string };

const NAV: NavDef[] = [
  { id: "knowledge", label: "Conhecimento", icon: "i-lucide-library" },
  { id: "characters", label: "Personagens", icon: "i-lucide-users-round" },
  { id: "places", label: "Lugares", icon: "i-lucide-map" },
  { id: "abilities", label: "Habilidades", icon: "i-lucide-sparkles" },
  { id: "settings", label: "Configurações", icon: "i-lucide-settings" },
];

export default function Sidebar() {
  return (
    <div class="app-pane w-236px flex-none bg-sidebar border-r border-border flex flex-col p-[22px_14px] box-border">
      {/*<button class="flex items-center gap-2.5 p-[2px_6px_18px]" onClick={() => actions.openInformationModal()}>
        <img src={logo} alt="Yellow Lore" class="w-28px h-28px rounded-full flex-none bg-transparent shadow-[0_3px_12px_-2px_rgba(0,0,0,0.45)]" />
        <div class="flex flex-col leading-none">
          <div class="font-display font-600 text-15.5px tracking-[0.06em] text-fg">YELLOW LORE</div>
          <div class="text-9.5px text-fg-muted tracking-[0.18em] uppercase mt-0.75">Codex de saber</div>
        </div>
      </button>*/}

      <VaultSwitcher />

      <div class="text-9px tracking-[0.22em] uppercase text-fg-muted font-600 px-2.5 mb-1.5 mt-1">
        Cânones
      </div>
      <div class="flex flex-col gap-0.5 flex-none">
        <For each={NAV}>
          {(item) => {
            const active = () => state.view === item.id;
            return (
              <div
                onClick={() => actions.setView(item.id)}
                class="relative flex items-center gap-3 px-2.5 py-[9px] rounded-8px cursor-pointer transition-colors duration-150"
                classList={{ "bg-accent-soft": active(), "hover:bg-hover": !active() }}
              >
                <div
                  class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] rounded-r-2px bg-accent transition-all duration-150"
                  classList={{ "h-4 opacity-100": active(), "h-0 opacity-0": !active() }}
                />
                <div
                  class={`${item.icon} w-4.5 h-4.5 flex-none`}
                  classList={{ "text-accent": active(), "text-fg-muted": !active() }}
                />
                <div
                  class="text-13.5px"
                  classList={{
                    "font-bold text-accent": active(),
                    "font-medium text-fg": !active(),
                  }}
                >
                  {item.label}
                </div>
              </div>
            );
          }}
        </For>
      </div>

      <div class="flex items-center justify-between px-2.5 mt-4 mb-1.5">
        <div class="text-9px tracking-[0.22em] uppercase text-fg-muted font-600">Chats</div>
        <div
          onClick={newChat}
          title="Nova conversa"
          class="i-lucide-plus w-3.5 h-3.5 text-fg-muted cursor-pointer hover:text-accent transition-colors duration-150"
        />
      </div>
      <div class="flex flex-col gap-0.5 flex-1 overflow-y-auto -mx-0.5 px-0.5">
        <For each={state.sessions}>
          {(s) => {
            const active = () => s.id === state.currentSessionId && state.view === "chat";
            return (
              <div
                onClick={() => openSession(s.id)}
                class="group flex items-center gap-2 px-2.5 py-[7px] rounded-8px cursor-pointer transition-colors duration-150"
                classList={{ "bg-accent-soft": active(), "hover:bg-hover": !active() }}
              >
                <div class="i-lucide-message-square w-3.5 h-3.5 flex-none" classList={{ "text-accent": active(), "text-fg-muted": !active() }} />
                <div class="flex-1 min-w-0 truncate text-12.5px" classList={{ "text-accent font-semibold": active(), "text-fg": !active() }}>{s.title}</div>
                <div onClick={(e) => renameSession(e, s.id, s.title)} class="i-lucide-pencil w-3 h-3 flex-none text-fg-muted opacity-0 group-hover:opacity-100 hover:text-fg" />
                <div onClick={(e) => removeSession(e, s.id, s.title)} class="i-lucide-trash-2 w-3 h-3 flex-none text-fg-muted opacity-0 group-hover:opacity-100 hover:text-danger" />
              </div>
            );
          }}
        </For>
        <Show when={state.sessions.length === 0}>
          <div class="text-11px text-fg-muted px-2.5 py-2 leading-[1.5]">
            Nenhuma conversa. Escreva no chat para criar.
          </div>
        </Show>
      </div>

      <Ornament class="mt-3 mb-3.5" />
      <div class="pt-0.5">
        <div
          onClick={actions.toggleTheme}
          class="flex items-center gap-2.5 px-2.5 py-[9px] rounded-8px cursor-pointer text-fg-muted text-13px transition-colors duration-150 hover:bg-hover"
        >
          <div
            class="w-4.5 h-4.5 flex-none text-fg-muted"
            classList={{ "i-lucide-sun": state.theme === "dark", "i-lucide-moon": state.theme !== "dark" }}
          />
          <div>{state.theme === "dark" ? "Modo claro" : "Modo escuro"}</div>
        </div>
      </div>
    </div>
  );
}
