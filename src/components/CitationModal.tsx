import { For, Show } from "solid-js";
import { state, actions } from "../store";

/// Shows the source quotes retrieved from a single document, opened by clicking
/// a citation chip under an assistant reply.
export default function CitationModal() {
  const c = () => state.citation;

  return (
    <Show when={c()}>
      <div
        onClick={actions.closeCitation}
        class="fixed inset-0 z-50 flex items-center justify-center p-6 anim-overlay"
        style={{ background: "oklch(0 0 0 / 0.5)" }}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          class="w-560px max-w-full max-h-[82vh] overflow-y-auto bg-panel border border-border rounded-16px p-7 box-border anim-pop"
        >
          <div class="flex items-start justify-between gap-4 mb-1.5">
            <div class="flex items-center gap-2.5 min-w-0">
              <div class="w-8 h-8 flex-none rounded-8px bg-accent-soft text-accent flex items-center justify-center">
                <div class="w-3 h-3 rounded-1px bg-current" />
              </div>
              <div class="min-w-0">
                <div class="font-serif text-18px font-600 truncate">{c()!.doc}</div>
                <div class="text-11.5px text-fg-muted">
                  {c()!.passages.length} trecho{c()!.passages.length === 1 ? "" : "s"} recuperado{c()!.passages.length === 1 ? "" : "s"}
                </div>
              </div>
            </div>
            <div
              onClick={actions.closeCitation}
              class="w-7 h-7 flex-none rounded-6px flex items-center justify-center cursor-pointer text-fg-muted text-16px transition-colors duration-150 hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </div>

          <div class="flex flex-col gap-2.5 mt-4">
            <For each={c()!.passages}>
              {(p, i) => (
                <div class="flex gap-3 rounded-10px bg-bg border border-border p-3.5">
                  <div class="text-11px font-bold font-mono text-fg-muted flex-none pt-0.5">
                    {String(i() + 1).padStart(2, "0")}
                  </div>
                  <div class="font-reading text-15px leading-[1.6] text-fg whitespace-pre-wrap">{p.text}</div>
                </div>
              )}
            </For>
          </div>
        </div>
      </div>
    </Show>
  );
}
