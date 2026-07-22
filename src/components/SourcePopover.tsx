import { Show } from "solid-js";
import { state } from "../store";

/// Hover-card for an inline `[N]` citation marker: shows the source document and
/// the retrieved passage. Non-interactive (click the marker to open the full
/// modal). Anchored below the marker, clamped to the viewport.
export default function SourcePopover() {
  const s = () => state.sourcePop;
  const left = () => Math.min(s()!.x, window.innerWidth - 340);
  const top = () => Math.min(s()!.y + 6, window.innerHeight - 20);

  return (
    <Show when={s()}>
      <div
        class="fixed z-[70] w-320px max-w-[80vw] pointer-events-none bg-panel border border-border rounded-12px p-3.5 shadow-lg anim-pop"
        style={{ left: `${left()}px`, top: `${top()}px` }}
      >
        <div class="flex items-center gap-2 mb-1.5">
          <div class="w-2 h-2 rounded-full flex-none bg-accent" />
          <span class="font-serif text-13.5px font-600 truncate">{s()!.doc}</span>
        </div>
        <div class="font-reading text-12.5px text-fg-muted leading-[1.55] max-h-140px overflow-hidden">
          {s()!.text}
        </div>
        <div class="text-10.5px text-fg-muted mt-2 opacity-70">Clique para ver a citação completa</div>
      </div>
    </Show>
  );
}
