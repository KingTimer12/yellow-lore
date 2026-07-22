import { For, Show } from "solid-js";
import { state, actions, type Source } from "../store";
import { providerLabel } from "../theme";
import Markdown from "./Markdown";
import Ornament from "./Ornament";

type Passage = { quote: string; text: string };

/// Collapse a flat source list into one entry per document, keeping its passages.
function groupSources(sources: Source[]): { doc: string; passages: Passage[] }[] {
  const map = new Map<string, Passage[]>();
  for (const s of sources) {
    const arr = map.get(s.doc) ?? [];
    arr.push({ quote: s.quote, text: s.text });
    map.set(s.doc, arr);
  }
  return [...map.entries()].map(([doc, passages]) => ({ doc, passages }));
}

export default function ChatView() {
  const summary = () =>
    `${state.settings.llmModel} · ${providerLabel(state.settings.llmProvider)}`;

  return (
    <div class="flex flex-col h-full box-border anim-view">
      <div class="flex items-center justify-between px-8 py-3.5 border-b border-border flex-none">
        <div class="font-serif text-24px font-600 tracking-[0.01em]">Chat</div>
        <div
          onClick={() => actions.setView("settings")}
          class="flex items-center gap-2 px-3 py-1.5 rounded-20px bg-accent-soft text-accent text-12px font-semibold cursor-pointer font-mono"
        >
          <div class="w-1.5 h-1.5 rounded-full bg-accent" />
          {summary()}
        </div>
      </div>

      <div class="flex-1 overflow-y-auto px-8 py-6.5 flex flex-col gap-4">
        <Show when={state.messages.length === 0 && !state.pending}>
          <div class="flex-1 flex flex-col items-center justify-center gap-3.5 text-center anim-fade">
            <div class="w-13 h-13 rounded-12px bg-accent-soft flex items-center justify-center">
              <span class="font-display text-accent text-20px font-700 leading-none pt-0.5">Y</span>
            </div>
            <div class="font-serif text-22px font-600">Interrogue seu mundo</div>
            <Ornament class="max-w-56px my-0.5" />
            <div class="font-reading text-15px text-fg-muted max-w-380px leading-[1.55]">
              Pergunte qualquer coisa sobre os documentos deste vault. Eu consulto a base antes de responder e cito os trechos-fonte.
            </div>
          </div>
        </Show>
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
                  <Show when={!isUser && m.thinking}>
                    <details class="mb-2.5 rounded-8px bg-hover border border-border overflow-hidden">
                      <summary class="px-3 py-2 text-11px font-semibold text-fg-muted uppercase tracking-[0.08em] cursor-pointer select-none flex items-center gap-2 list-none">
                        <span class="think-caret text-9px">▸</span> Raciocínio
                      </summary>
                      <div class="px-3 pb-2.5 pt-0.5 text-12.5px text-fg-muted leading-[1.55] whitespace-pre-wrap border-t border-border">
                        {m.thinking}
                      </div>
                    </details>
                  </Show>
                  <Show
                    when={isUser}
                    fallback={
                      <Show
                        when={m.text}
                        fallback={
                          <div class="flex items-center gap-1.5 py-0.5">
                            <div class="typing-dot" />
                            <div class="typing-dot" />
                            <div class="typing-dot" />
                          </div>
                        }
                      >
                        <Markdown source={m.text} mentions sources={m.sources} />
                      </Show>
                    }
                  >
                    <div class="whitespace-pre-wrap">{m.text}</div>
                  </Show>
                  <Show when={state.settings.showSources && m.sources && m.sources.length > 0}>
                    <div
                      class="mt-2.5 pt-2.5 flex flex-wrap gap-1.5"
                      style={{ "border-top": `1px solid ${isUser ? "oklch(1 0 0 / 0.25)" : "var(--border)"}` }}
                    >
                      <For each={groupSources(m.sources!)}>
                        {(g) => (
                          <div
                            onClick={() => actions.openCitation(g.doc, g.passages)}
                            class="flex items-center gap-1.5 px-2.5 py-1 rounded-7px text-11.5px font-semibold cursor-pointer border transition-colors duration-150"
                            classList={{
                              "border-transparent bg-white/15 hover:bg-white/25 text-accent-fg": isUser,
                              "border-border bg-hover hover:border-accent hover:text-accent text-fg-muted": !isUser,
                            }}
                          >
                            <div class="w-2 h-2 rounded-1px bg-current flex-none opacity-70" />
                            <span class="truncate max-w-200px">{g.doc}</span>
                            <span class="opacity-60">{g.passages.length}</span>
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
      </div>

      <div class="px-8 pt-2 pb-6 border-t border-border flex gap-3 flex-none">
        <input
          value={state.chatInput}
          disabled={state.pending}
          onInput={(e) => actions.setChatInput(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === "Enter") actions.sendMessage(); }}
          placeholder="Pergunte algo sobre seus documentos..."
          class="flex-1 px-4 py-3 rounded-10px border border-border bg-panel text-fg text-14px outline-none transition-colors"
        />
        <Show
          when={state.pending}
          fallback={
            <button
              onClick={() => actions.sendMessage()}
              disabled={!state.chatInput.trim()}
              aria-label="Enviar"
              class="flex items-center justify-center px-4 py-3 rounded-10px bg-accent text-accent-fg text-14px font-bold cursor-pointer whitespace-nowrap border-none transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <i class="i-lucide-send w-4.5 h-4.5" />
            </button>
          }
        >
          <button
            onClick={() => actions.stopGeneration()}
            aria-label="Parar"
            class="flex items-center gap-2 px-4 py-3 rounded-10px bg-danger text-white text-14px font-bold cursor-pointer whitespace-nowrap border-none transition-all active:scale-95"
          >
            <i class="i-lucide-square w-4 h-4" /> Parar
          </button>
        </Show>
      </div>
    </div>
  );
}
