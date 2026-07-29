import { For, Show, createMemo, createSignal, onMount, type JSX } from "solid-js";
import { state, actions } from "../store";
import { api, isTauri, type CacheStats } from "../api";
import { EMBED_PROVIDERS, LLM_PROVIDERS, type ProviderMeta } from "../theme";

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

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
  const usesGemini = createMemo(
    () => state.settings.llmProvider === "gemini" || state.settings.embeddingProvider === "gemini",
  );

  // Cache size is read from the backend, not the store: it changes as calls run,
  // and nothing else in the UI needs it.
  const [cache, setCache] = createSignal<CacheStats>({ entries: 0, bytes: 0 });
  onMount(() => {
    if (isTauri) api.cacheStats().then(setCache).catch(() => {});
  });
  function clearCache() {
    actions.askConfirm({
      title: "Limpar o cache de respostas?",
      message:
        "As próximas chamadas iguais voltam a gastar requisição do provedor. Nada do seu vault é apagado.",
      confirmLabel: "Limpar",
      danger: true,
      onConfirm: () => {
        api.clearCache().then(setCache).catch((e) => actions.notify(`${e}`, "Cache"));
      },
    });
  }

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
        modelPlaceholder="llama3.1 / gpt-4o / gemini-2.5-flash"
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
      <Show when={usesGemini()}>
        <div class="flex flex-col gap-4">
          <Divider label="Gemini" />
          <Field label="API Key" type="password" value={state.settings.geminiApiKey} onInput={(v) => actions.setSetting("geminiApiKey", v)} />
          <Field label="Base URL" value={state.settings.geminiBaseUrl} onInput={(v) => actions.setSetting("geminiBaseUrl", v)} placeholder="https://generativelanguage.googleapis.com/v1beta/openai" />
          <div class="text-11.5px text-fg-muted">
            Pegue a chave no Google AI Studio. Usa a camada compatível com OpenAI do Gemini, então streaming e citações funcionam igual. O raciocínio dos modelos 2.5 fica desligado enquanto "Mostrar raciocínio" estiver desmarcado.
          </div>
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
          on={state.settings.rerank}
          onToggle={() => actions.setSetting("rerank", !state.settings.rerank)}
          label="Reranking de trechos por relevância (LLM)"
          hint="Após a busca híbrida, uma chamada de LLM reordena os trechos recuperados por relevância antes de responder — corta ruído do top-k. Custa uma chamada a mais por pergunta."
        />
        <Toggle
          on={state.settings.corrective}
          onToggle={() => actions.setSetting("corrective", !state.settings.corrective)}
          label="RAG corretivo — auto-avaliar e re-buscar (CRAG)"
          hint="Após rascunhar a resposta, o modelo avalia se ela resolve a pergunta; se não, refaz a busca com uma rede mais ampla e responde de novo (uma única re-tentativa). Mais preciso, porém mais lento e com chamadas de LLM extras. Bom para o modo precisão."
        />
        <Toggle
          on={state.settings.showThinking}
          onToggle={() => actions.setSetting("showThinking", !state.settings.showThinking)}
          label="Deixar a resposta final raciocinar (thinking)"
          hint="Off por padrão: para resumos/consultas o raciocínio é desnecessário e, com modelos que pensam em texto puro, vaza um preâmbulo longo na resposta. Passos internos do RAG nunca raciocinam."
        />
      </div>

      {/* Entity extraction */}
      <div class="flex flex-col gap-4">
        <Divider label="Extração de entidades" />
        <Field
          label="Modelo de extração (opcional)"
          value={state.settings.extractionModel}
          onInput={(v) => actions.setSetting("extractionModel", v)}
          placeholder="vazio = usa o modelo de LLM do chat"
        />
        <div class="text-11.5px text-fg-muted -mt-2">
          Deixe vazio para reutilizar o modelo de chat (sem baixar nem carregar um segundo modelo). Aponte para um modelo menor/rápido só se tiver VRAM sobrando.
        </div>
        <Slider
          label={`Janela por chamada (${(state.settings.extractionWindowChars / 1000).toFixed(0)}k caracteres)`}
          min={4000}
          max={80000}
          step={2000}
          value={state.settings.extractionWindowChars}
          onInput={(v) => actions.setSetting("extractionWindowChars", v)}
        />
        <div class="text-11.5px text-fg-muted -mt-2">
          Cada janela é uma requisição ao modelo: dobrar o tamanho corta o número de requisições pela metade. 12k é seguro para modelos locais pequenos; com modelos em nuvem de contexto grande, suba bastante — é o jeito mais direto de caber num limite de poucas requisições por dia. Janelas grandes demais para o modelo pioram a extração (ele começa a esquecer o meio do texto).
        </div>
        <Slider
          label={`Janelas em paralelo (${state.settings.extractionConcurrency})`}
          min={1}
          max={8}
          step={1}
          value={state.settings.extractionConcurrency}
          onInput={(v) => actions.setSetting("extractionConcurrency", v)}
        />
        <div class="text-11.5px text-fg-muted -mt-2">
          1 = sequencial (seguro para uma GPU local — o Ollama serializa mesmo). Aumente só com provedores em nuvem (OpenAI/vLLM), onde chamadas paralelas cortam o tempo. Em GPU local, valores altos podem estourar a memória.
        </div>
        <Toggle
          on={state.settings.dedupEntities}
          onToggle={() => actions.setSetting("dedupEntities", !state.settings.dedupEntities)}
          label="Unificar entidades duplicadas via LLM na extração"
          hint="Passo extra que mescla apelidos/nomes parciais (ex.: “Cesar” = “Cesar Magnus”), inclusive contra entidades já salvas. Só os nomes ambíguos são enviados ao LLM."
        />
        <Show when={state.settings.dedupEntities}>
          <Toggle
            on={state.settings.dedupWithContext}
            onToggle={() => actions.setSetting("dedupWithContext", !state.settings.dedupWithContext)}
            label="Enviar contexto junto dos nomes na unificação"
            hint="Manda também o resumo de uma linha e as relações diretas de cada candidato, o que permite detectar duplicata cujos nomes não têm nada em comum. Custa cerca de dez vezes mais tokens nessa chamada e, como fundir duas pessoas diferentes é pior que deixar uma duplicata, vem desligado."
          />
        </Show>
      </div>

      {/* Response cache */}
      <div class="flex flex-col gap-4">
        <Divider label="Cache de respostas" />
        <Toggle
          on={state.settings.cacheLlm}
          onToggle={() => actions.setSetting("cacheLlm", !state.settings.cacheLlm)}
          label="Reaproveitar respostas idênticas do provedor"
          hint="Guarda em disco a resposta de cada chamada, com chave na requisição exata (provedor, modelo, temperatura e prompt). Repetir a extração de um capítulo, reindexar ou refazer a mesma pergunta passa a não gastar requisição nenhuma. Como qualquer mudança no prompt muda a chave, um acerto é sempre a resposta certa para aquela pergunta."
        />
        <Show when={state.settings.cacheLlm}>
          <div class="flex items-center justify-between gap-3 px-3.5 py-2.5 rounded-8px bg-hover border border-border">
            <div class="text-11.5px text-fg-muted font-mono">
              {cache().entries} {cache().entries === 1 ? "resposta guardada" : "respostas guardadas"} · {formatBytes(cache().bytes)}
            </div>
            <button
              onClick={clearCache}
              disabled={cache().entries === 0}
              class="px-3 py-1.5 rounded-7px border border-border bg-panel text-11.5px font-semibold cursor-pointer transition-colors hover:text-danger hover:border-danger disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Limpar cache
            </button>
          </div>
        </Show>
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
      <div class="grid grid-cols-2 lg:grid-cols-4 gap-2.5">
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
