import { winCtl, maximized } from "../window";
import logo from "../assets/logo.svg";
import { actions } from "../store";

/// Custom window decoration (native title bar is disabled). The bar is a Tauri
/// drag region; the buttons are not, so they stay clickable.
export default function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      class="titlebar flex items-center gap-2.5 h-9 flex-none pl-3.5 pr-2 bg-sidebar border-b border-border select-none"
    >
      <button
        onClick={() => actions.openInformationModal()}
        aria-label="Sobre o Yellow Lore"
        class="flex items-center gap-2.5 rounded-6px px-1.5 py-1 bg-transparent border-none cursor-pointer transition-colors duration-150 hover:bg-hover"
      >
        <img src={logo} alt="" class="w-4.5 h-4.5 rounded-full flex-none pointer-events-none" />
        <div class="text-11px font-display tracking-[0.16em] uppercase text-fg-muted pointer-events-none">
          Yellow Lore
        </div>
      </button>
      <div data-tauri-drag-region class="flex-1 self-stretch" />

      <div class="flex items-center gap-0.5">
        <button
          onClick={() => winCtl.minimize()}
          aria-label="Minimizar"
          class="w-8 h-7 flex items-center justify-center rounded-6px bg-transparent border-none text-fg-muted cursor-pointer transition-colors duration-150 hover:bg-hover hover:text-fg"
        >
          <div class="i-lucide-minus text-14px" />
        </button>
        <button
          onClick={() => winCtl.toggleMaximize()}
          aria-label="Maximizar"
          class="w-8 h-7 flex items-center justify-center rounded-6px bg-transparent border-none text-fg-muted cursor-pointer transition-colors duration-150 hover:bg-hover hover:text-fg"
        >
          <div class="text-12px" classList={{ "i-lucide-copy": maximized(), "i-lucide-square": !maximized() }} />
        </button>
        <button
          onClick={() => winCtl.close()}
          aria-label="Fechar"
          class="w-8 h-7 flex items-center justify-center rounded-6px bg-transparent border-none text-fg-muted cursor-pointer transition-colors duration-150 hover:bg-danger hover:text-white"
        >
          <div class="i-lucide-x text-14px" />
        </button>
      </div>
    </div>
  );
}
