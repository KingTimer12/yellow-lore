import { For, Show, createSignal } from "solid-js";
import { state, actions } from "../store";

/// Vault picker: each vault is an isolated knowledge base (one story / obra).
export default function VaultSwitcher() {
  const [open, setOpen] = createSignal(false);
  const active = () => state.vaults.find((v) => v.id === state.activeVaultId);

  function newVault() {
    actions.openVaultModal();
    setOpen(false);
  }
  function rename(id: string, current: string) {
    actions.askPrompt({
      title: "Renomear vault",
      defaultValue: current,
      placeholder: "Nome do vault",
      onSubmit: (name) => void actions.renameVault(id, name),
    });
  }
  function remove(id: string, name: string) {
    actions.askConfirm({
      title: "Excluir vault?",
      message: `Excluir o vault "${name}" e todo o seu conhecimento? Esta ação não pode ser desfeita.`,
      confirmLabel: "Excluir",
      danger: true,
      onConfirm: () => void actions.deleteVault(id),
    });
  }

  return (
    <div class="relative mb-5">
      <div
        onClick={() => setOpen((o) => !o)}
        class="flex items-center gap-2 px-2.5 py-2 rounded-8px cursor-pointer bg-panel border border-border transition-colors duration-150 hover:border-fg-muted"
      >
        <div class="w-2 h-2 rounded-2px bg-accent flex-none" />
        <div class="flex-1 min-w-0 text-12.5px font-semibold truncate">{active()?.name ?? "—"}</div>
        <div class="text-fg-muted text-10px flex-none" classList={{ "rotate-180": open() }}>▾</div>
      </div>

      <Show when={open()}>
        <div class="absolute left-0 right-0 top-full mt-1.5 z-20 bg-panel border border-border rounded-10px p-1.5 shadow-lg anim-pop">
          <For each={state.vaults}>
            {(v) => (
              <div
                class="group flex items-center gap-2 px-2 py-1.5 rounded-6px cursor-pointer transition-colors duration-150 hover:bg-hover"
                classList={{ "bg-accent-soft": v.id === state.activeVaultId }}
                onClick={() => { actions.selectVault(v.id); setOpen(false); }}
              >
                <div
                  class="flex-1 min-w-0 text-12.5px truncate"
                  classList={{ "text-accent font-semibold": v.id === state.activeVaultId }}
                >
                  {v.name}
                </div>
                <div
                  class="hidden group-hover:block text-10px text-fg-muted hover:text-fg px-1"
                  onClick={(e) => { e.stopPropagation(); rename(v.id, v.name); }}
                >
                  editar
                </div>
                <div
                  class="hidden group-hover:block text-12px text-fg-muted hover:text-danger px-1"
                  onClick={(e) => { e.stopPropagation(); remove(v.id, v.name); }}
                >
                  ×
                </div>
              </div>
            )}
          </For>
          <div class="h-px bg-border my-1" />
          <div
            onClick={newVault}
            class="px-2 py-1.5 rounded-6px cursor-pointer text-12.5px text-accent font-semibold transition-colors duration-150 hover:bg-hover"
          >
            + Novo vault
          </div>
        </div>
      </Show>
    </div>
  );
}
