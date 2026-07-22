import { For, Show } from "solid-js";
import { state } from "../store";

/// Hover-card shown when the pointer is over an entity name inside a chat reply.
/// Non-interactive (pointer-events: none) so it never steals the hover — it just
/// surfaces the notes the user/extraction recorded for that character/place.
export default function MentionPopover() {
  const m = () => state.mention;
  // Anchor below the mention, clamped so it never overflows the right edge.
  const left = () => Math.min(m()!.x, window.innerWidth - 320);
  const top = () => Math.min(m()!.y + 6, window.innerHeight - 20);

  return (
    <Show when={m()}>
      <div
        class="fixed z-[70] w-300px max-w-[80vw] pointer-events-none bg-panel border border-border rounded-12px p-3.5 shadow-lg anim-pop"
        style={{ left: `${left()}px`, top: `${top()}px` }}
      >
        <div class="flex items-center gap-2 mb-1">
          <span
            class="w-2 h-2 rounded-full flex-none"
            style={{ background: m()!.kind === "place" ? "oklch(0.68 0.11 195)" : "var(--accent)" }}
          />
          <span class="font-serif text-15px font-600 leading-tight">{m()!.name}</span>
          <Show when={m()!.role}>
            <span class="text-11px text-fg-muted truncate">· {m()!.role}</span>
          </Show>
        </div>
        <Show when={m()!.summary} fallback={<div class="text-12px text-fg-muted italic">Sem anotações.</div>}>
          <div class="text-12.5px text-fg-muted leading-[1.5]">{m()!.summary}</div>
        </Show>
        <Show when={m()!.traits.length > 0}>
          <div class="flex gap-1.5 flex-wrap mt-2">
            <For each={m()!.traits}>
              {(t) => <span class="px-2 py-0.5 rounded-20px bg-hover text-10.5px text-fg-muted">{t}</span>}
            </For>
          </div>
        </Show>
      </div>
    </Show>
  );
}
