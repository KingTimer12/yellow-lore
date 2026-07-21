import { Show } from "solid-js";
import {
  updateInfo,
  updateBusy,
  updateProgress,
  updateError,
  updateDismissed,
  installUpdate,
  dismissUpdate,
} from "../update";

/// Slim banner shown when a newer signed build is available. Offers a one-click
/// download-install-relaunch, with live progress; dismissible until next launch.
export default function UpdateBanner() {
  const pct = () => Math.round(updateProgress() * 100);
  return (
    <Show when={updateInfo() && !updateDismissed()}>
      <div class="flex items-center gap-3 px-4 py-2 bg-accent-soft border-b border-border text-13px flex-none anim-fade">
        <i class="i-lucide-download w-4.5 h-4.5 text-accent flex-none" />
        <div class="flex-1 min-w-0">
          <Show
            when={!updateBusy()}
            fallback={
              <span class="text-fg-muted">
                Baixando atualização… {pct()}%
              </span>
            }
          >
            <span class="text-fg">
              Nova versão <span class="font-semibold text-accent">v{updateInfo()!.version}</span> disponível.
            </span>
          </Show>
          <Show when={updateError()}>
            <span class="text-danger ml-2">Falhou: {updateError()}</span>
          </Show>
        </div>

        <Show when={updateBusy()}>
          <div class="w-28 h-1.5 rounded-full bg-hover overflow-hidden flex-none">
            <div class="h-full bg-accent transition-all duration-150" style={{ width: `${pct()}%` }} />
          </div>
        </Show>

        <Show when={!updateBusy()}>
          <button
            onClick={() => void installUpdate()}
            class="px-3 py-1.5 rounded-7px bg-accent text-accent-fg text-12px font-bold cursor-pointer border-none whitespace-nowrap transition-transform active:scale-95"
          >
            {updateError() ? "Tentar novamente" : "Atualizar e reiniciar"}
          </button>
          <button
            onClick={() => dismissUpdate()}
            aria-label="Dispensar"
            class="flex items-center justify-center w-6 h-6 rounded-6px bg-transparent border-none cursor-pointer text-fg-muted hover:text-fg hover:bg-hover transition-colors"
          >
            <i class="i-lucide-x w-3.5 h-3.5" />
          </button>
        </Show>
      </div>
    </Show>
  );
}
