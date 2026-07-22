import { Show } from "solid-js";
import { state, actions } from "../store";

export default function EditDrawer() {
  const form = () => state.editForm;
  const isCharacter = () => state.editing?.kind === "character";
  const isCreating = () => !!state.editing?.creating;
  const noun = () => (isCharacter() ? "personagem" : "lugar");
  const title = () => (isCreating() ? `Adicionar ${noun()}` : `Editar ${noun()}`);
  const secondaryLabel = () => (isCharacter() ? "Papel" : "Tipo");

  return (
    <Show when={state.editing && form()}>
      <div
        onClick={actions.closeEdit}
        class="absolute inset-0 flex justify-end z-10 anim-overlay"
        style={{ background: "oklch(0 0 0 / 0.5)" }}
      >
        <div
          onClick={(e) => e.stopPropagation()}
          class="w-420px h-full bg-panel border-l border-border p-7 box-border overflow-y-auto anim-drawer"
        >
          <div class="flex items-center justify-between mb-5.5">
            <div class="font-serif text-20px font-600">{title()}</div>
            <div
              onClick={actions.closeEdit}
              class="w-7 h-7 rounded-6px flex items-center justify-center cursor-pointer text-fg-muted text-16px transition-colors duration-150 hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </div>

          <div class="flex flex-col gap-4">
            <Field label="Nome" value={form()!.name} onInput={(v) => actions.setEditField("name", v)} />
            <Field label={secondaryLabel()} value={form()!.role} onInput={(v) => actions.setEditField("role", v)} />

            <div>
              <label class="text-11.5px font-semibold text-fg-muted uppercase tracking-[0.04em]">Resumo</label>
              <textarea
                value={form()!.summary}
                onInput={(e) => actions.setEditField("summary", e.currentTarget.value)}
                class="w-full mt-1.5 px-3 py-2.5 rounded-8px border border-border bg-bg text-fg text-13.5px leading-[1.5] box-border outline-none resize-y min-h-90px"
              />
            </div>

            <Show when={isCharacter()}>
              <Field
                label="Traços (separados por vírgula)"
                value={form()!.traitsText}
                onInput={(v) => actions.setEditField("traitsText", v)}
              />
            </Show>

            <Show when={!isCreating() && form()!.sourceDoc}>
              <div class="bg-hover rounded-10px p-3.5 flex flex-col gap-1.5">
                <div class="text-11px font-bold text-fg-muted uppercase tracking-[0.04em]">Fonte original</div>
                <div class="text-12.5px font-semibold">{form()!.sourceDoc}</div>
                <div class="text-12.5px text-fg-muted italic leading-[1.5]">{form()!.sourceQuote}</div>
              </div>
            </Show>
          </div>

          <div class="flex gap-2.5 mt-6.5">
            <div
              onClick={actions.closeEdit}
              class="flex-1 text-center py-3 rounded-8px border border-border text-13.5px font-semibold cursor-pointer text-fg-muted transition-transform active:scale-95 hover:bg-hover"
            >
              Cancelar
            </div>
            <div
              onClick={actions.saveEdit}
              class="flex-1 text-center py-3 rounded-8px bg-accent text-accent-fg text-13.5px font-bold cursor-pointer transition-transform active:scale-95"
            >
              {isCreating() ? "Adicionar" : "Salvar"}
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
}

function Field(props: { label: string; value: string; onInput: (v: string) => void }) {
  return (
    <div>
      <label class="text-11.5px font-semibold text-fg-muted uppercase tracking-[0.04em]">{props.label}</label>
      <input
        value={props.value}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        class="w-full mt-1.5 px-3 py-2.5 rounded-8px border border-border bg-bg text-fg text-14px box-border outline-none"
      />
    </div>
  );
}
