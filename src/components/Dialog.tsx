import { Show } from "solid-js";
import { state, actions } from "../store";

/// Centralized alert / confirm dialog. Replaces the browser's native `alert`
/// and `confirm` so notifications and destructive confirmations match the app's
/// look and stay theme-aware. Driven by `state.dialog`.
export default function Dialog() {
  const d = () => state.dialog;
  const isConfirm = () => d()!.mode === "confirm";

  return (
    <Show when={d()}>
      <div
        onClick={() => actions.resolveDialog(false)}
        class="fixed inset-0 z-[60] flex items-center justify-center p-6 anim-overlay"
        style={{ background: "oklch(0 0 0 / 0.5)" }}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          class="w-420px max-w-full bg-panel border border-border rounded-16px p-6 box-border anim-pop"
        >
          <div class="font-serif text-18px font-600 mb-1.5">{d()!.title}</div>
          <div class="font-reading text-14.5px text-fg-muted leading-[1.55] whitespace-pre-wrap">
            {d()!.message}
          </div>

          <div class="flex items-center justify-end gap-2.5 mt-6">
            <Show when={isConfirm()}>
              <button
                onClick={() => actions.resolveDialog(false)}
                class="px-4 py-2 rounded-8px border border-border bg-panel text-fg text-13px font-semibold cursor-pointer transition-colors hover:bg-hover"
              >
                {d()!.cancelLabel}
              </button>
            </Show>
            <button
              onClick={() => actions.resolveDialog(true)}
              class="px-4 py-2 rounded-8px text-13px font-bold cursor-pointer border-none transition-transform active:scale-95"
              classList={{
                "bg-danger text-white": d()!.danger,
                "bg-accent text-accent-fg": !d()!.danger,
              }}
            >
              {d()!.confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}
