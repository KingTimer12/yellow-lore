import { For, Show, createMemo, createSignal } from "solid-js";
import { state, actions } from "../store";
import { AVATAR_HUES, initials } from "../theme";
import Graph from "./Graph";

export default function CharactersView() {
  // Filter the grid by name, role (título) or personality trait — essential once
  // a large work yields dozens of characters. Summary is matched too as a bonus.
  const filtered = createMemo(() => {
    const q = state.charactersFilter.trim().toLowerCase();
    if (!q) return state.characters;
    return state.characters.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.role.toLowerCase().includes(q) ||
        c.summary.toLowerCase().includes(q) ||
        c.traits.some((t) => t.toLowerCase().includes(q)),
    );
  });

  return (
    <div class="p-8 overflow-y-auto h-full box-border flex flex-col gap-5.5 anim-view">
      <div class="flex items-center justify-between flex-wrap gap-3.5">
        <div>
          <div class="font-serif text-24px font-600 tracking-[0.01em]">Personagens</div>
          <div class="text-13px text-fg-muted mt-1">
            Extraídos da base — veja o grafo de personagens e lugares, ou edite os cards
          </div>
        </div>
        <div class="flex items-center gap-2.5">
          <div class="flex bg-panel border border-border rounded-9px p-0.75">
            <For each={[["grid", "Cards"], ["graph", "Grafo"]] as const}>
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
            onClick={() => actions.openCreate("character")}
            class="px-3.5 py-2 rounded-8px border border-border bg-panel text-fg text-12.5px font-bold cursor-pointer transition-transform active:scale-95 hover:border-accent"
          >
            + Adicionar
          </button>
          <button
            onClick={() => actions.extractEntities(false)}
            disabled={state.extracting}
            title="Extrai apenas dos documentos novos"
            class="px-3.5 py-2 rounded-8px bg-accent text-accent-fg text-12.5px font-bold cursor-pointer border-none transition-transform active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {state.extracting ? "Extraindo..." : "Extrair novos"}
          </button>
          <button
            onClick={() =>
              actions.askConfirm({
                title: "Re-extrair tudo?",
                message: "Re-processa TODOS os documentos deste vault. Personagens editados ou adicionados por você são preservados.",
                confirmLabel: "Re-extrair",
                onConfirm: () => actions.extractEntities(true),
              })
            }
            disabled={state.extracting}
            title="Re-processa todos os documentos (preserva editados/adicionados)"
            class="px-2.5 py-2 rounded-8px border border-border bg-panel text-fg-muted text-12.5px font-semibold cursor-pointer transition-colors hover:text-fg disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Tudo
          </button>
        </div>
      </div>

      <Show when={state.charactersTab === "grid" && state.characters.length > 0}>
        <div class="relative">
          <input
            value={state.charactersFilter}
            onInput={(e) => actions.setCharactersFilter(e.currentTarget.value)}
            placeholder="Buscar por nome, título ou traço de personalidade…"
            class="w-full px-3.5 py-2.5 pr-9 rounded-9px border border-border bg-panel text-fg text-13.5px box-border outline-none transition-colors focus:border-accent"
          />
          <Show when={state.charactersFilter}>
            <div
              onClick={() => actions.setCharactersFilter("")}
              title="Limpar"
              class="absolute right-2.5 top-1/2 -translate-y-1/2 w-6 h-6 flex items-center justify-center rounded-6px text-fg-muted text-15px cursor-pointer hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </Show>
        </div>
      </Show>

      <Show when={state.charactersTab === "grid" && state.characters.length === 0}>
        <div class="border-1.5 border-dashed border-border rounded-14px p-10 flex flex-col items-center gap-2.5 text-fg-muted text-center">
          <div class="text-13.5px font-semibold">Nenhum personagem ainda</div>
          <div class="text-12px max-w-360px">
            Clique em <b>Extrair da base</b> para o assistente ler os documentos deste vault e identificar personagens automaticamente.
          </div>
        </div>
      </Show>

      <Show when={state.charactersTab === "grid" && state.characters.length > 0 && filtered().length === 0}>
        <div class="border-1.5 border-dashed border-border rounded-14px p-8 flex flex-col items-center gap-1.5 text-fg-muted text-center">
          <div class="text-13.5px font-semibold">Nenhum resultado</div>
          <div class="text-12px">Nada corresponde a “{state.charactersFilter}”.</div>
        </div>
      </Show>

      <Show when={state.charactersTab === "grid" && filtered().length > 0}>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
          <For each={filtered()}>
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
                    <div class="font-serif text-17px font-600 leading-tight">{c.name}</div>
                    <div class="text-12px text-fg-muted">{c.role}</div>
                  </div>
                  <div
                    class="px-2 py-0.75 rounded-6px text-10px font-bold font-mono flex-none"
                    classList={{
                      "bg-accent-soft text-accent": c.status === "Extraído",
                      "bg-success-soft text-success": c.status === "Editado",
                      "bg-hover text-fg": c.status === "Adicionado",
                    }}
                  >
                    {c.status}
                  </div>
                </div>
                <div class="text-13px text-fg-muted leading-[1.5]">{c.summary}</div>
                <div class="flex gap-1.5 flex-wrap content-start mt-auto min-h-72px">
                  <For each={c.traits}>
                    {(trait) => (
                      <div class="px-2.25 py-0.75 rounded-20px bg-hover text-11px text-fg-muted h-fit">
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

      <Show when={state.charactersTab === "graph"}>
        <div class="flex-1 min-h-460px">
          <Graph />
        </div>
        <RelationsEditor />
      </Show>
    </div>
  );
}

// ---- Manual relation editing ----------------------------------------------
//
// Auto-extraction gets sloppier as more chapters pile up; curating edges by hand
// keeps the graph (and GraphRAG retrieval) accurate. Edges are keyed by their
// (from, to, label) triple — editing a label = delete + re-add.
function RelationsEditor() {
  const names = createMemo(() => {
    const set = new Set<string>();
    for (const c of state.characters) set.add(c.name);
    for (const p of state.places) set.add(p.name);
    return [...set].sort((a, b) => a.localeCompare(b));
  });

  const [from, setFrom] = createSignal("");
  const [to, setTo] = createSignal("");
  const [label, setLabel] = createSignal("");

  const canAdd = () =>
    from().trim() && to().trim() && from().toLowerCase() !== to().toLowerCase();

  const add = () => {
    if (!canAdd()) return;
    actions.addRelation(from(), to(), label());
    setLabel("");
  };

  const selectClass =
    "px-2.5 py-2 rounded-8px border border-border bg-panel text-fg text-12.5px outline-none min-w-0";

  return (
    <div class="border border-border rounded-14px bg-panel p-4 flex flex-col gap-3">
      <div class="text-12px font-bold text-fg-muted uppercase tracking-[0.04em]">
        Relações do grafo
      </div>
      <div class="text-11.5px text-fg-muted -mt-1.5 leading-[1.45]">
        Adicione ou remova ligações manualmente. Quanto mais capítulos, mais a extração automática erra — relações curadas por você melhoram a precisão do GraphRAG.
      </div>

      {/* Add form */}
      <div class="flex items-center gap-2 flex-wrap">
        <select class={selectClass} value={from()} onChange={(e) => setFrom(e.currentTarget.value)}>
          <option value="">De…</option>
          <For each={names()}>{(n) => <option value={n}>{n}</option>}</For>
        </select>
        <input
          class={selectClass + " flex-1"}
          placeholder="relação (ex.: mentor de, irmão de)"
          value={label()}
          onInput={(e) => setLabel(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <select class={selectClass} value={to()} onChange={(e) => setTo(e.currentTarget.value)}>
          <option value="">Para…</option>
          <For each={names()}>{(n) => <option value={n}>{n}</option>}</For>
        </select>
        <button
          onClick={add}
          disabled={!canAdd()}
          class="px-3.5 py-2 rounded-8px bg-accent text-accent-fg text-12.5px font-bold cursor-pointer border-none transition-transform active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          Adicionar
        </button>
      </div>

      {/* Existing relations */}
      <Show
        when={state.relations.length > 0}
        fallback={<div class="text-12px text-fg-muted">Nenhuma relação ainda.</div>}
      >
        <div class="flex flex-col gap-1.5 max-h-220px overflow-y-auto">
          <For each={state.relations}>
            {(r) => (
              <div class="flex items-center gap-2 text-12.5px py-1 px-2 rounded-7px hover:bg-hover group">
                <span class="font-semibold">{r.from}</span>
                <span class="text-fg-muted">—{r.label ? ` ${r.label} ` : " "}→</span>
                <span class="font-semibold">{r.to}</span>
                <button
                  onClick={() => actions.removeRelation(r)}
                  title="Remover relação"
                  class="ml-auto w-5 h-5 flex-none rounded-full text-fg-muted text-13px leading-none cursor-pointer border-none bg-transparent hover:text-danger transition-colors"
                >
                  ×
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
