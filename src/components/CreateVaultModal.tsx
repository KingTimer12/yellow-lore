import { For, Show, createMemo, createSignal } from "solid-js";
import { state, actions } from "../store";
import { EMBED_PROVIDERS, LLM_PROVIDERS, type ProviderMeta } from "../theme";

/// First-run modal: name the vault and set the minimum provider info needed to
/// use it. Provider fields write straight to settings and are persisted on
/// confirm; full tuning still lives in Configurações.
export default function CreateVaultModal() {
  const [name, setName] = createSignal("");
  const s = () => state.settings;

  const usesOpenAI = createMemo(() => s().llmProvider === "openai" || s().embeddingProvider === "openai");
  const usesOllama = createMemo(() => s().llmProvider === "ollama" || s().embeddingProvider === "ollama");
  const usesVllm = createMemo(() => s().llmProvider === "vllm" || s().embeddingProvider === "vllm");

  const canCreate = () => name().trim().length > 0;

  function close() {
    setName("");
    actions.closeVaultModal();
  }
  async function confirm() {
    if (!canCreate()) return;
    await actions.confirmCreateVault(name());
    setName("");
  }

  return (
    <Show when={state.vaultModalOpen}>
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
            <div class="font-serif text-21px font-600">Criar vault</div>
            <div
              onClick={close}
              class="w-7 h-7 rounded-6px flex items-center justify-center cursor-pointer text-fg-muted text-16px transition-colors duration-150 hover:bg-hover hover:text-fg"
            >
              ×
            </div>
          </div>
          <div class="text-12.5px text-fg-muted leading-[1.5] mb-5.5">
            Uma base de conhecimento isolada. Dê um nome e escolha o provedor de IA — você pode ajustar o resto em Configurações.
          </div>

          <div class="flex flex-col gap-5">
            <div>
              <label class="text-11.5px font-semibold text-fg-muted uppercase tracking-[0.04em]">Nome do vault</label>
              <input
                value={name()}
                autofocus
                onInput={(e) => setName(e.currentTarget.value)}
                onKeyDown={(e) => { if (e.key === "Enter") confirm(); }}
                placeholder="Ex.: Crônicas de Vharren"
                class="w-full mt-1.5 px-3 py-2.5 rounded-8px border border-border bg-bg text-fg text-14px box-border outline-none transition-colors"
              />
            </div>

            <ProviderPick
              title="Modelo de LLM (geração)"
              providers={LLM_PROVIDERS}
              selected={s().llmProvider}
              onSelect={(id) => actions.setSetting("llmProvider", id)}
              model={s().llmModel}
              onModel={(v) => actions.setSetting("llmModel", v)}
              modelPlaceholder="llama3.1 / gpt-4o"
            />

            <ProviderPick
              title="Modelo de embedding (busca)"
              providers={EMBED_PROVIDERS}
              selected={s().embeddingProvider}
              onSelect={(id) => actions.setSetting("embeddingProvider", id)}
              model={s().embeddingModel}
              onModel={(v) => actions.setSetting("embeddingModel", v)}
              modelPlaceholder="nomic-embed-text / text-embedding-3-small"
            />

            <Show when={usesOpenAI()}>
              <div class="flex flex-col gap-3">
                <Divider label="OpenAI" />
                <Field label="API Key" type="password" value={s().openaiApiKey} onInput={(v) => actions.setSetting("openaiApiKey", v)} />
              </div>
            </Show>
            <Show when={usesOllama()}>
              <div class="flex flex-col gap-3">
                <Divider label="Ollama" />
                <Field label="Endpoint local" value={s().ollamaEndpoint} onInput={(v) => actions.setSetting("ollamaEndpoint", v)} placeholder="http://localhost:11434" />
              </div>
            </Show>
            <Show when={usesVllm()}>
              <div class="flex flex-col gap-3">
                <Divider label="vLLM" />
                <Field label="Base URL" value={s().vllmBaseUrl} onInput={(v) => actions.setSetting("vllmBaseUrl", v)} placeholder="http://localhost:8000/v1" />
                <Field label="API Key (opcional)" type="password" value={s().vllmApiKey} onInput={(v) => actions.setSetting("vllmApiKey", v)} />
              </div>
            </Show>
          </div>

          <div class="flex gap-2.5 mt-7">
            <div
              onClick={close}
              class="flex-1 text-center py-3 rounded-8px border border-border text-13.5px font-semibold cursor-pointer text-fg-muted transition-transform active:scale-95 hover:bg-hover"
            >
              Cancelar
            </div>
            <button
              onClick={confirm}
              disabled={!canCreate()}
              class="flex-1 py-3 rounded-8px bg-accent text-accent-fg text-13.5px font-bold cursor-pointer border-none transition-transform active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Criar vault
            </button>
          </div>
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
