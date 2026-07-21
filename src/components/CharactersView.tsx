import { For, Show } from "solid-js";
import { state, actions } from "../store";
import { AVATAR_HUES, initials } from "../theme";

function hueForName(name: string): string {
  const i = state.characters.findIndex((c) => c.name === name);
  return AVATAR_HUES[(i < 0 ? 0 : i) % AVATAR_HUES.length];
}

export default function CharactersView() {
  return (
    <div class="p-8 overflow-y-auto h-full box-border flex flex-col gap-5.5 anim-view">
      <div class="flex items-center justify-between flex-wrap gap-3.5">
        <div>
          <div class="text-18px font-bold">Personagens</div>
          <div class="text-13px text-fg-muted mt-1">
            Extraídos automaticamente da base — edite para corrigir
          </div>
        </div>
        <div class="flex items-center gap-2.5">
          <div class="flex bg-panel border border-border rounded-9px p-0.75">
            <For each={[["grid", "Cards"], ["relations", "Relações"]] as const}>
              {([tab, label]) => (
                <div
                  onClick={() => actions.setCharactersTab(tab)}
                  class="px-3.5 py-1.5 rounded-7px text-12.5px font-semibold cursor-pointer transition-colors duration-150"
                  classList={{
                    "bg-accent-soft text-accent": state.charactersTab === tab,
                    "text-fg-muted": state.charactersTab !== tab,
                  }}
                >
                  {label}
                </div>
              )}
            </For>
          </div>
          <button
            onClick={() => actions.extractEntities()}
            disabled={state.extracting}
            class="px-3.5 py-2 rounded-8px bg-accent text-accent-fg text-12.5px font-bold cursor-pointer border-none transition-transform active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {state.extracting ? "Extraindo..." : "Extrair da base"}
          </button>
        </div>
      </div>

      <Show when={state.charactersTab === "grid" && state.characters.length === 0}>
        <div class="border-1.5 border-dashed border-border rounded-14px p-10 flex flex-col items-center gap-2.5 text-fg-muted text-center">
          <div class="text-13.5px font-semibold">Nenhum personagem ainda</div>
          <div class="text-12px max-w-360px">
            Clique em <b>Extrair da base</b> para o assistente ler os documentos deste vault e identificar personagens automaticamente.
          </div>
        </div>
      </Show>

      <Show when={state.charactersTab === "grid" && state.characters.length > 0}>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
          <For each={state.characters}>
            {(c, i) => (
              <div
                onClick={() => actions.openEdit("character", c.id)}
                class="bg-panel border border-border rounded-14px p-5 cursor-pointer flex flex-col gap-3 anim-stagger transition-all duration-150 hover:border-accent hover:-translate-y-0.5"
                style={{ "animation-delay": `${i() * 45}ms` }}
              >
                <div class="flex items-center gap-3">
                  <div
                    class="w-10 h-10 flex-none rounded-full text-white flex items-center justify-center font-bold text-14px"
                    style={{ background: AVATAR_HUES[i() % AVATAR_HUES.length] }}
                  >
                    {initials(c.name)}
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="text-14.5px font-bold">{c.name}</div>
                    <div class="text-12px text-fg-muted">{c.role}</div>
                  </div>
                  <div
                    class="px-2 py-0.75 rounded-6px text-10px font-bold font-mono flex-none"
                    classList={{
                      "bg-accent-soft text-accent": c.status === "Extraído",
                      "bg-success-soft text-success": c.status === "Editado",
                    }}
                  >
                    {c.status}
                  </div>
                </div>
                <div class="text-13px text-fg-muted leading-[1.5]">{c.summary}</div>
                <div class="flex gap-1.5 flex-wrap">
                  <For each={c.traits}>
                    {(trait) => (
                      <div class="px-2.25 py-0.75 rounded-20px bg-hover text-11px text-fg-muted">
                        {trait}
                      </div>
                    )}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Show when={state.charactersTab === "relations"}>
        <div class="flex flex-col gap-2.5">
          <For each={state.relations}>
            {(r, i) => (
              <div
                class="flex items-center gap-3.5 bg-panel border border-border rounded-12px px-5 py-4 anim-stagger"
                style={{ "animation-delay": `${i() * 45}ms` }}
              >
                <div
                  class="w-8 h-8 flex-none rounded-full text-white flex items-center justify-center text-12px font-bold"
                  style={{ background: hueForName(r.from) }}
                >
                  {initials(r.from)}
                </div>
                <div class="text-13.5px font-semibold flex-none">{r.from}</div>
                <div class="flex-1 h-px bg-border relative">
                  <div class="absolute -top-2.5 left-1/2 -translate-x-1/2 bg-bg px-2 text-11.5px text-accent font-semibold whitespace-nowrap">
                    {r.label}
                  </div>
                </div>
                <div class="text-13.5px font-semibold flex-none">{r.to}</div>
                <div
                  class="w-8 h-8 flex-none rounded-full text-white flex items-center justify-center text-12px font-bold"
                  style={{ background: hueForName(r.to) }}
                >
                  {initials(r.to)}
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
