import { For, type JSX } from "solid-js";
import { state, actions, type View } from "../store";
import VaultSwitcher from "./VaultSwitcher";

type NavDef = {
  id: View;
  label: string;
  shape: "circle-filled" | "square-filled" | "diamond" | "circle-outline" | "square-outline";
};

const NAV: NavDef[] = [
  { id: "chat", label: "Chat", shape: "circle-filled" },
  { id: "knowledge", label: "Conhecimento", shape: "square-filled" },
  { id: "characters", label: "Personagens", shape: "diamond" },
  { id: "places", label: "Lugares", shape: "circle-outline" },
  { id: "settings", label: "Configurações", shape: "square-outline" },
];

function iconStyle(shape: NavDef["shape"], active: boolean): JSX.CSSProperties {
  const color = active ? "var(--accent)" : "var(--fg-muted)";
  const base: JSX.CSSProperties = { width: "9px", height: "9px" };
  switch (shape) {
    case "circle-filled":
      return { ...base, "border-radius": "50%", background: color };
    case "square-filled":
      return { ...base, "border-radius": "2px", background: color };
    case "diamond":
      return { ...base, "border-radius": "2px", background: color, transform: "rotate(45deg)" };
    case "circle-outline":
      return { ...base, "border-radius": "50%", border: `1.5px solid ${color}` };
    case "square-outline":
      return { ...base, "border-radius": "2px", border: `1.5px solid ${color}` };
  }
}

export default function Sidebar() {
  return (
    <div class="w-236px flex-none bg-sidebar border-r border-border flex flex-col p-[22px_14px] box-border">
      <div class="flex items-center gap-2.5 p-[2px_8px_18px]">
        <div class="w-26px h-26px rounded-7px bg-accent flex-none" />
        <div class="font-bold text-15px tracking-[-0.2px]">Yellow Lore</div>
      </div>

      <VaultSwitcher />

      <div class="flex flex-col gap-0.5 flex-1">
        <For each={NAV}>
          {(item) => {
            const active = () => state.view === item.id;
            return (
              <div
                onClick={() => actions.setView(item.id)}
                class="flex items-center gap-3 px-2.5 py-[9px] rounded-8px cursor-pointer transition-colors duration-150"
                classList={{ "bg-accent-soft": active(), "hover:bg-hover": !active() }}
              >
                <div class="w-5 h-5 flex-none flex items-center justify-center">
                  <div style={iconStyle(item.shape, active())} />
                </div>
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

      <div class="border-t border-border pt-3.5">
        <div
          onClick={actions.toggleTheme}
          class="flex items-center gap-2.5 px-2.5 py-[9px] rounded-8px cursor-pointer text-fg-muted text-13px transition-colors duration-150 hover:bg-hover"
        >
          <div class="w-5 h-5 flex-none flex items-center justify-center">
            <div class="w-13px h-13px rounded-full border-1.5 border-fg-muted" />
          </div>
          <div>{state.theme === "dark" ? "Modo claro" : "Modo escuro"}</div>
        </div>
      </div>
    </div>
  );
}
