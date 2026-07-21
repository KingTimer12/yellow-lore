import { For, Show } from "solid-js";
import { state, actions } from "../store";
import { providerLabel } from "../theme";

export default function ChatView() {
  const summary = () =>
    `${state.settings.llmModel} · ${providerLabel(state.settings.llmProvider)}`;

  return (
    <div class="flex flex-col h-full box-border anim-view">
      <div class="flex items-center justify-between px-8 py-5 border-b border-border flex-none">
        <div class="text-18px font-bold">Chat</div>
        <div
          onClick={() => actions.setView("settings")}
          class="flex items-center gap-2 px-3 py-1.5 rounded-20px bg-accent-soft text-accent text-12px font-semibold cursor-pointer font-mono"
        >
          <div class="w-1.5 h-1.5 rounded-full bg-accent" />
          {summary()}
        </div>
      </div>

      <div class="flex-1 overflow-y-auto px-8 py-6.5 flex flex-col gap-4">
        <For each={state.messages}>
          {(m) => {
            const isUser = m.role === "user";
            return (
              <div class="flex" classList={{ "justify-end": isUser, "justify-start": !isUser }}>
                <div
                  class="max-w-640px px-4 py-3.25 rounded-14px text-14px leading-[1.55] anim-msg"
                  classList={{
                    "bg-accent text-accent-fg": isUser,
                    "bg-panel text-fg border border-border": !isUser,
                  }}
                >
                  <div>{m.text}</div>
                  <Show when={state.settings.showSources && m.sources && m.sources.length > 0}>
                    <div
                      class="mt-2.5 pt-2.5 flex flex-col gap-1.5"
                      style={{ "border-top": `1px solid ${isUser ? "oklch(1 0 0 / 0.25)" : "var(--border)"}` }}
                    >
                      <For each={m.sources}>
                        {(src) => (
                          <div
                            class="text-11.5px font-mono leading-[1.5]"
                            style={{ color: isUser ? "oklch(1 0 0 / 0.85)" : "var(--fg-muted)" }}
                          >
                            <span class="font-bold">{src.doc}</span> — {src.quote}
                          </div>
                        )}
                      </For>
                    </div>
                  </Show>
                </div>
              </div>
            );
          }}
        </For>
        <Show when={state.pending}>
          <div class="flex justify-start anim-fade">
            <div class="flex items-center gap-1.5 px-4 py-3.25 rounded-14px bg-panel border border-border text-fg-muted">
              <div class="typing-dot" />
              <div class="typing-dot" />
              <div class="typing-dot" />
            </div>
          </div>
        </Show>
      </div>

      <div class="px-8 pt-4.5 pb-6 border-t border-border flex gap-3 flex-none">
        <input
          value={state.chatInput}
          disabled={state.pending}
          onInput={(e) => actions.setChatInput(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === "Enter") actions.sendMessage(); }}
          placeholder="Pergunte algo sobre seus documentos..."
          class="flex-1 px-4 py-3.25 rounded-10px border border-border bg-panel text-fg text-14px outline-none transition-colors"
        />
        <button
          onClick={() => actions.sendMessage()}
          disabled={state.pending || !state.chatInput.trim()}
          class="px-6 py-3.25 rounded-10px bg-accent text-accent-fg text-14px font-bold cursor-pointer whitespace-nowrap border-none transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Enviar
        </button>
      </div>
    </div>
  );
}
