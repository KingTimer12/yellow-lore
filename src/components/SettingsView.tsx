import { For, Show, createMemo, type JSX } from "solid-js";
import { state, actions } from "../store";
import { EMBED_PROVIDERS, LLM_PROVIDERS, type ProviderMeta } from "../theme";

export default function SettingsView() {
  const usesOpenAI = createMemo(
    () => state.settings.llmProvider === "openai" || state.settings.embeddingProvider === "openai",
  );
  const usesOllama = createMemo(
    () => state.settings.llmProvider === "ollama" || state.settings.embeddingProvider === "ollama",
  );
  const usesVllm = createMemo(
    () => state.settings.llmProvider === "vllm" || state.settings.embeddingProvider === "vllm",
  );

  return (
    <div class="p-8 overflow-y-auto overflow-x-hidden h-full w-full box-border flex flex-col gap-7.5 anim-view">
      <div>
        <div class="font-serif text-24px font-600 tracking-[0.01em]">Configurações</div>
        <div class="text-13px text-fg-muted mt-1">
          LLM e embedding são configurados separadamente. O agente sempre busca na base antes de responder.
        </div>
      </div>

      {/* LLM */}
      <ProviderSection
        title="Modelo de LLM (geração)"
        providers={LLM_PROVIDERS}
        selected={state.settings.llmProvider}
        onSelect={(id) => actions.setSetting("llmProvider", id)}
        model={state.settings.llmModel}
        onModel={(v) => actions.setSetting("llmModel", v)}
        modelLabel="Modelo de LLM"
        modelPlaceholder="llama3.1 / gpt-4o"
      />

      {/* Embedding */}
      <ProviderSection
        title="Modelo de embedding (busca)"
        providers={EMBED_PROVIDERS}
        selected={state.settings.embeddingProvider}
        onSelect={(id) => actions.setSetting("embeddingProvider", id)}
        model={state.settings.embeddingModel}
        onModel={(v) => actions.setSetting("embeddingModel", v)}
        modelLabel="Modelo de embedding"
        modelPlaceholder="nomic-embed-text / text-embedding-3-small"
      />

      {/* Credentials */}
      <Show when={usesOpenAI()}>
        <div class="flex flex-col gap-4">
          <Divider label="OpenAI" />
          <Field label="API Key" type="password" value={state.settings.openaiApiKey} onInput={(v) => actions.setSetting("openaiApiKey", v)} />
          <Field label="Base URL" value={state.settings.openaiBaseUrl} onInput={(v) => actions.setSetting("openaiBaseUrl", v)} />
        </div>
      </Show>
      <Show when={usesOllama()}>
        <div class="flex flex-col gap-4">
          <Divider label="Ollama" />
          <Field label="Endpoint local" value={state.settings.ollamaEndpoint} onInput={(v) => actions.setSetting("ollamaEndpoint", v)} placeholder="http://localhost:11434" />
          <Slider label={`Contexto — num_ctx (${state.settings.ollamaNumCtx})`} min={2048} max={32768} step={2048} value={state.settings.ollamaNumCtx} onInput={(v) => actions.setSetting("ollamaNumCtx", v)} />
          <div class="text-11.5px text-fg-muted">Janela de contexto do Ollama. Modelos que "pensam" muito precisam de mais para não cortar a resposta no meio; reduza para poupar RAM/VRAM.</div>
        </div>
      </Show>
      <Show when={usesVllm()}>
        <div class="flex flex-col gap-4">
          <Divider label="vLLM" />
          <Field label="Base URL" value={state.settings.vllmBaseUrl} onInput={(v) => actions.setSetting("vllmBaseUrl", v)} placeholder="http://localhost:8000/v1" />
          <Field label="API Key (opcional)" type="password" value={state.settings.vllmApiKey} onInput={(v) => actions.setSetting("vllmApiKey", v)} />
        </div>
      </Show>

      {/* System prompt */}
      <div class="flex flex-col gap-4">
        <Divider label="Prompt do sistema (agente)" />
        <textarea
          value={state.settings.systemPrompt}
          onInput={(e) => actions.setSetting("systemPrompt", e.currentTarget.value)}
          class="w-full px-3 py-2.5 rounded-8px border border-border bg-panel text-fg text-13.5px leading-[1.5] box-border outline-none resize-y min-h-110px transition-colors"
        />
        <div class="text-11.5px text-fg-muted">
          Ajuste como o assistente deve responder. Os trechos recuperados são anexados a este prompt automaticamente.
        </div>
      </div>

      {/* RAG behaviour */}
      <div class="flex flex-col gap-4">
        <Divider label="Comportamento do RAG" />
        <div class="grid grid-cols-3 gap-5">
          <Slider label={`Chunk (${state.settings.chunkSize} tk)`} min={200} max={2000} step={50} value={state.settings.chunkSize} onInput={(v) => actions.setSetting("chunkSize", v)} />
          <Slider label={`Overlap (${state.settings.chunkOverlap} tk)`} min={0} max={400} step={20} value={state.settings.chunkOverlap} onInput={(v) => actions.setSetting("chunkOverlap", v)} />
          <Slider label={`Top-k (${state.settings.topK})`} min={1} max={12} step={1} value={state.settings.topK} onInput={(v) => actions.setSetting("topK", v)} />
          <Slider label={`Temperatura (${state.settings.temperature.toFixed(1)})`} min={0} max={1} step={0.1} value={state.settings.temperature} onInput={(v) => actions.setSetting("temperature", v)} />
        </div>
        <Toggle
          on={state.settings.showSources}
          onToggle={() => actions.setSetting("showSources", !state.settings.showSources)}
          label="Mostrar trechos-fonte nas respostas do chat"
        />
        <Toggle
          on={state.settings.dedupEntities}
          onToggle={() => actions.setSetting("dedupEntities", !state.settings.dedupEntities)}
          label="Unificar entidades duplicadas via LLM na extração"
          hint="Passo extra que mescla apelidos/nomes parciais (ex.: “Cesar” = “Cesar Magnus”). Uma chamada de LLM a mais por extração."
        />
      </div>

      <div class="flex items-center gap-3.5">
        <button
          onClick={() => actions.saveSettings()}
          class="px-6 py-3 rounded-8px bg-accent text-accent-fg text-13.5px font-bold cursor-pointer border-none transition-transform active:scale-95"
        >
          Salvar configurações
        </button>
        <Show when={state.savedToast}>
          <div class="text-12.5px text-success anim-fade">Configurações salvas.</div>
        </Show>
      </div>
    </div>
  );
}

// ---- pieces ---------------------------------------------------------------

function ProviderSection(props: {
  title: string;
  providers: ProviderMeta[];
  selected: string;
  onSelect: (id: string) => void;
  model: string;
  onModel: (v: string) => void;
  modelLabel: string;
  modelPlaceholder: string;
}) {
  return (
    <div class="flex flex-col gap-3">
      <div class="text-12px font-bold text-fg-muted uppercase tracking-[0.04em]">{props.title}</div>
      <div class="grid grid-cols-3 gap-2.5">
        <For each={props.providers}>
          {(p) => {
            const active = () => p.id === props.selected;
            return (
              <div
                onClick={() => props.onSelect(p.id)}
                class="p-3.5 rounded-10px cursor-pointer border-1.5 transition-all duration-150"
                classList={{ "border-accent bg-accent-soft": active(), "border-border bg-panel hover:border-fg-muted": !active() }}
              >
                <div class="text-13.5px font-bold">{p.label}</div>
                <div class="text-11.5px text-fg-muted mt-0.75 leading-[1.4]">{p.hint}</div>
              </div>
            );
          }}
        </For>
      </div>
      <Field label={props.modelLabel} value={props.model} onInput={props.onModel} placeholder={props.modelPlaceholder} />
    </div>
  );
}

function Toggle(props: { on: boolean; onToggle: () => void; label: string; hint?: string }) {
  return (
    <div>
      <div onClick={props.onToggle} class="flex items-center gap-2.5 cursor-pointer">
        <div
          class="w-9 h-5 rounded-10px relative flex-none transition-colors duration-150"
          classList={{ "bg-accent": props.on, "bg-border": !props.on }}
        >
          <div
            class="w-4 h-4 rounded-full bg-white absolute top-0.5 transition-all duration-150"
            classList={{ "left-4.5": props.on, "left-0.5": !props.on }}
          />
        </div>
        <div class="text-13px">{props.label}</div>
      </div>
      <Show when={props.hint}>
        <div class="text-11.5px text-fg-muted mt-1 ml-11.5 leading-[1.45]">{props.hint}</div>
      </Show>
    </div>
  );
}

function Divider(props: { label: string }) {
  return (
    <div class="flex items-center gap-3">
      <div class="text-12px font-bold text-fg-muted uppercase tracking-[0.04em] whitespace-nowrap">{props.label}</div>
      <div class="flex-1 h-px bg-border" />
    </div>
  );
}

function Field(props: {
  label: string;
  value: string;
  onInput: (v: string) => void;
  type?: string;
  placeholder?: string;
}) {
  return (
    <div>
      <label class="text-11.5px font-semibold text-fg-muted uppercase tracking-[0.04em]">{props.label}</label>
      <input
        type={props.type ?? "text"}
        value={props.value}
        placeholder={props.placeholder ?? ""}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        class="w-full mt-1.5 px-3 py-2.5 rounded-8px border border-border bg-panel text-fg text-14px box-border outline-none transition-colors"
      />
    </div>
  );
}

function Slider(props: {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onInput: (v: number) => void;
}): JSX.Element {
  return (
    <div>
      <label class="text-12.5px text-fg-muted">{props.label}</label>
      <input
        type="range"
        min={props.min}
        max={props.max}
        step={props.step}
        value={props.value}
        onInput={(e) => props.onInput(Number(e.currentTarget.value))}
        class="w-full mt-2.5"
        style={{ "accent-color": "var(--accent)" }}
      />
    </div>
  );
}
