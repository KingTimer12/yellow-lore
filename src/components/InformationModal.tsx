import { For, Show } from "solid-js";
import { state, actions } from "../store";
import { type ProviderMeta } from "../theme";

/// First-run modal: name the vault and set the minimum provider info needed to
/// use it. Provider fields write straight to settings and are persisted on
/// confirm; full tuning still lives in Configurações.
export default function InformationModal() {
  function close() {
    actions.closeInformationModal();
  }

  return (
    <Show when={state.informationModalOpen}>
      <div
        onClick={close}
        class="fixed inset-0 z-50 flex items-center justify-center p-6 anim-overlay"
        style={{ background: "oklch(0 0 0 / 0.5)" }}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          class="w-540px max-w-full max-h-[88vh] overflow-y-auto bg-panel border border-border rounded-16px p-7 box-border anim-pop"
        >
          <div class="flex items-center justify-between mb-1.5">
            <div class="font-serif text-21px font-600">Yellow Lore</div>
            <div
              onClick={close}
              class="w-7 h-7 rounded-6px flex items-center justify-center cursor-pointer text-fg-muted text-16px transition-colors duration-150 hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </div>
          <p>Versão: 0.1.0</p>
          <p>Desenvolvido por: Aaron King</p>
        </div>
      </div>
    </Show>
  );
}

// ---- pieces ---------------------------------------------------------------

function ProviderPick(props: {
  title: string;
  providers: ProviderMeta[];
  selected: string;
  onSelect: (id: string) => void;
  model: string;
  onModel: (v: string) => void;
  modelPlaceholder: string;
}) {
  return (
    <div class="flex flex-col gap-2.5">
      <div class="text-11.5px font-bold text-fg-muted uppercase tracking-[0.04em]">{props.title}</div>
      <div class="grid grid-cols-3 gap-2">
        <For each={props.providers}>
          {(p) => {
            const active = () => p.id === props.selected;
            return (
              <div
                onClick={() => props.onSelect(p.id)}
                class="px-2.5 py-2 rounded-8px cursor-pointer border-1.5 text-center transition-all duration-150"
                classList={{ "border-accent bg-accent-soft": active(), "border-border bg-bg hover:border-fg-muted": !active() }}
              >
                <div class="text-12.5px font-bold">{p.label}</div>
              </div>
            );
          }}
        </For>
      </div>
      <input
        value={props.model}
        placeholder={props.modelPlaceholder}
        onInput={(e) => props.onModel(e.currentTarget.value)}
        class="w-full px-3 py-2.5 rounded-8px border border-border bg-bg text-fg text-14px box-border outline-none transition-colors"
      />
    </div>
  );
}

function Divider(props: { label: string }) {
  return (
    <div class="flex items-center gap-3">
      <div class="text-11.5px font-bold text-fg-muted uppercase tracking-[0.04em] whitespace-nowrap">{props.label}</div>
      <div class="flex-1 h-px bg-border" />
    </div>
  );
}

function Field(props: { label: string; value: string; onInput: (v: string) => void; type?: string; placeholder?: string }) {
  return (
    <div>
      <label class="text-11.5px font-semibold text-fg-muted uppercase tracking-[0.04em]">{props.label}</label>
      <input
        type={props.type ?? "text"}
        value={props.value}
        placeholder={props.placeholder ?? ""}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        class="w-full mt-1.5 px-3 py-2.5 rounded-8px border border-border bg-bg text-fg text-14px box-border outline-none transition-colors"
      />
    </div>
  );
}
