import { For, Show, createMemo } from "solid-js";
import { state, actions } from "../store";

// Powers / skills (e.g. "Previsão", "Hipótese") — proper nouns that are NOT
// characters. Own canon tab, mirrors the Places layout.
export default function AbilitiesView() {
  const filtered = createMemo(() => {
    const q = state.abilitiesFilter.trim().toLowerCase();
    const list = q
      ? state.abilities.filter(
          (a) =>
            a.name.toLowerCase().includes(q) ||
            a.type.toLowerCase().includes(q) ||
            a.summary.toLowerCase().includes(q),
        )
      : state.abilities;
    return [...list].sort((a, b) => a.name.localeCompare(b.name, "pt"));
  });

  function remove(e: MouseEvent, id: string, name: string) {
    e.stopPropagation();
    actions.askConfirm({
      title: "Excluir habilidade?",
      message: `Excluir "${name}"? Também remove as relações dela no grafo. Esta ação não pode ser desfeita.`,
      confirmLabel: "Excluir",
      danger: true,
      onConfirm: () => void actions.deleteEntity("ability", id),
    });
  }

  return (
    <div class="p-8 overflow-y-auto h-full box-border flex flex-col gap-5.5 anim-view">
      <div class="flex items-center justify-between flex-wrap gap-3.5">
        <div>
          <div class="font-serif text-24px font-600 tracking-[0.01em]">Habilidades</div>
          <div class="text-13px text-fg-muted mt-1">
            Poderes, magias e técnicas extraídos da base — edite para corrigir
          </div>
        </div>
        <div class="flex items-center gap-2.5">
          <button
            onClick={() => actions.openCreate("ability")}
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
                message: "Re-processa TODOS os documentos deste vault. Habilidades editadas ou adicionadas por você são preservadas.",
                confirmLabel: "Re-extrair",
                onConfirm: () => actions.extractEntities(true),
              })
            }
            disabled={state.extracting}
            title="Re-processa todos os documentos (preserva editadas/adicionadas)"
            class="px-2.5 py-2 rounded-8px border border-border bg-panel text-fg-muted text-12.5px font-semibold cursor-pointer transition-colors hover:text-fg disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Tudo
          </button>
        </div>
      </div>

      <Show when={state.abilities.length > 0}>
        <div class="relative">
          <input
            value={state.abilitiesFilter}
            onInput={(e) => actions.setAbilitiesFilter(e.currentTarget.value)}
            placeholder="Buscar por nome, tipo ou descrição…"
            class="w-full px-3.5 py-2.5 pr-9 rounded-9px border border-border bg-panel text-fg text-13.5px box-border outline-none transition-colors focus:border-accent"
          />
          <Show when={state.abilitiesFilter}>
            <div
              onClick={() => actions.setAbilitiesFilter("")}
              title="Limpar"
              class="absolute right-2.5 top-1/2 -translate-y-1/2 w-6 h-6 flex items-center justify-center rounded-6px text-fg-muted text-15px cursor-pointer hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </Show>
        </div>
      </Show>

      <Show when={state.abilities.length === 0}>
        <div class="border-1.5 border-dashed border-border rounded-14px p-10 flex flex-col items-center gap-2.5 text-fg-muted text-center">
          <div class="text-13.5px font-semibold">Nenhuma habilidade ainda</div>
          <div class="text-12px max-w-360px">
            Clique em <b>Extrair novos</b> para identificar poderes, magias e técnicas nos documentos deste vault.
          </div>
        </div>
      </Show>

      <Show when={state.abilities.length > 0 && filtered().length === 0}>
        <div class="border-1.5 border-dashed border-border rounded-14px p-8 flex flex-col items-center gap-1.5 text-fg-muted text-center">
          <div class="text-13.5px font-semibold">Nenhum resultado</div>
          <div class="text-12px">Nada corresponde a “{state.abilitiesFilter}”.</div>
        </div>
      </Show>

      <div class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
        <For each={filtered()}>
          {(a, i) => (
            <div
              onClick={() => actions.openEdit("ability", a.id)}
              class="group bg-panel border border-border rounded-14px p-5 cursor-pointer flex flex-col gap-3 anim-stagger transition-all duration-150 hover:border-accent hover:-translate-y-0.5"
              style={{ "animation-delay": `${i() * 45}ms` }}
            >
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 flex-none rounded-10px bg-accent-soft flex items-center justify-center">
                  <div class="i-lucide-sparkles w-5 h-5 text-accent" />
                </div>
                <div class="flex-1 min-w-0">
                  <div class="font-serif text-17px font-600 leading-tight">{a.name}</div>
                  <div class="text-12px text-fg-muted">{a.type}</div>
                </div>
                <div
                  class="px-2 py-0.75 rounded-6px text-10px font-bold font-mono flex-none"
                  classList={{
                    "bg-accent-soft text-accent": a.status === "Extraído",
                    "bg-success-soft text-success": a.status === "Editado",
                    "bg-hover text-fg": a.status === "Adicionado",
                  }}
                >
                  {a.status}
                </div>
                <div
                  onClick={(e) => remove(e, a.id, a.name)}
                  title="Excluir habilidade"
                  class="i-lucide-trash-2 w-4 h-4 flex-none text-fg-muted opacity-0 group-hover:opacity-100 hover:text-danger transition-opacity"
                />
              </div>
              <div class="text-13px text-fg-muted leading-[1.5]">{a.summary}</div>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
