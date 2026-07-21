import { Show, onCleanup, onMount } from "solid-js";
import { state, actions } from "../store";
import { appVersion } from "../version";
import Ornament from "./Ornament";
import logo from "../assets/logo.svg";

/// "Sobre" modal: identity, version and credits. Opened from the sidebar brand.
export default function InformationModal() {
  return (
    <Show when={state.informationModalOpen}>
      <ModalBody />
    </Show>
  );
}

function ModalBody() {
  const close = () => actions.closeInformationModal();

  // Esc closes — the listener only lives while the modal is mounted.
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    onCleanup(() => window.removeEventListener("keydown", onKey));
  });

  return (
    <div
      onClick={close}
      class="fixed inset-0 z-50 flex items-center justify-center p-6 anim-overlay"
      style={{ background: "oklch(0 0 0 / 0.5)", "backdrop-filter": "blur(3px)" }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Sobre o Yellow Lore"
        onClick={(e) => e.stopPropagation()}
        class="relative w-420px max-w-full max-h-[88vh] overflow-y-auto bg-panel border border-border rounded-16px px-8 pt-9 pb-7 box-border anim-pop shadow-[0_24px_70px_-18px_rgba(0,0,0,0.65)]"
      >
        <button
          onClick={close}
          aria-label="Fechar"
          class="absolute top-3.5 right-3.5 w-7 h-7 rounded-6px flex items-center justify-center cursor-pointer bg-transparent border-none text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
        >
          <div class="i-lucide-x text-15px" />
        </button>

        {/* Brand */}
        <div class="flex flex-col items-center text-center gap-3">
          <img
            src={logo}
            alt="Yellow Lore"
            class="w-16 h-16 rounded-full flex-none bg-transparent shadow-[0_6px_20px_-4px_rgba(0,0,0,0.5)]"
          />
          <div class="flex flex-col items-center gap-1">
            <div class="font-display text-24px font-700 tracking-[0.08em] uppercase">Yellow Lore</div>
            <div class="text-10.5px text-fg-muted tracking-[0.22em] uppercase">Codex de saber</div>
          </div>
          <Ornament class="max-w-72px my-0.5" />
          <p class="font-reading text-14px text-fg-muted leading-[1.6] max-w-320px">
            Base de conhecimento com IA. Guarde suas obras em vaults isolados e
            interrogue-as — o assistente sempre busca na base antes de responder.
          </p>
        </div>

        {/* Meta */}
        <div class="mt-6 flex flex-col rounded-12px border border-border overflow-hidden">
          <InfoRow label="Versão" value={appVersion()} />
          <InfoRow label="Autor" value="Aaron King" />
          <InfoRow label="Licença" value="MIT" />
          <InfoRow label="Stack" value="SolidJS · UnoCSS · Tauri · SQLite" last />
        </div>

        <button
          onClick={close}
          class="mt-6 w-full py-3 rounded-8px bg-accent text-accent-fg text-13.5px font-bold cursor-pointer border-none transition-transform active:scale-95"
        >
          Fechar
        </button>
      </div>
    </div>
  );
}

function InfoRow(props: { label: string; value: string; last?: boolean }) {
  return (
    <div
      class="flex items-center justify-between gap-4 px-4 py-3 bg-bg"
      classList={{ "border-b border-border": !props.last }}
    >
      <div class="text-11px font-bold text-fg-muted uppercase tracking-[0.06em] flex-none">
        {props.label}
      </div>
      <div class="text-13px text-fg text-right">{props.value}</div>
    </div>
  );
}
