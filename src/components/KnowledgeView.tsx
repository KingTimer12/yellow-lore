import { For, Show, createMemo, createSignal } from "solid-js";
import { state, actions } from "../store";

export default function KnowledgeView() {
  const filtered = createMemo(() => {
    const q = state.docsFilter.toLowerCase();
    return state.docs.filter((d) => d.name.toLowerCase().includes(q));
  });

  let fileInput: HTMLInputElement | undefined;
  const [busy, setBusy] = createSignal(false);
  const [err, setErr] = createSignal("");

  async function handleFiles(files: FileList | null) {
    if (!files || !files.length) return;
    setErr("");
    setBusy(true);
    try {
      for (const file of Array.from(files)) {
        const content = await file.text();
        await actions.ingestDocument(file.name, content);
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
      if (fileInput) fileInput.value = "";
    }
  }

  return (
    <div class="p-8 overflow-y-auto h-full box-border flex flex-col gap-5.5 anim-view">
      <div class="flex items-center justify-between flex-wrap gap-3.5">
        <div>
          <div class="text-18px font-bold">Base de conhecimento</div>
          <div class="text-13px text-fg-muted mt-1">Documentos indexados para busca (RAG)</div>
        </div>
        <input
          value={state.docsFilter}
          onInput={(e) => actions.setDocsFilter(e.currentTarget.value)}
          placeholder="Buscar documento..."
          class="px-3.5 py-2.25 rounded-8px border border-border bg-panel text-fg text-13px w-220px outline-none"
        />
      </div>

      <input
        ref={fileInput}
        type="file"
        multiple
        accept=".txt,.md,.markdown,text/*"
        class="hidden"
        onChange={(e) => handleFiles(e.currentTarget.files)}
      />
      <div
        onClick={() => fileInput?.click()}
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => { e.preventDefault(); handleFiles(e.dataTransfer?.files ?? null); }}
        class="border-1.5 border-dashed border-border rounded-14px p-8.5 flex flex-col items-center gap-2.5 text-fg-muted cursor-pointer transition-colors duration-150 hover:border-accent hover:text-accent"
        classList={{ "opacity-60 pointer-events-none": busy() }}
      >
        <div class="w-34px h-34px rounded-10px border-2 border-current" />
        <div class="text-13.5px font-semibold">
          {busy() ? "Indexando..." : "Arraste arquivos aqui ou clique para selecionar"}
        </div>
        <div class="text-12px">TXT, Markdown (PDF/DOCX em breve)</div>
      </div>
      <Show when={err()}>
        <div class="text-12.5px text-danger">{err()}</div>
      </Show>

      <div class="flex flex-col border border-border rounded-12px overflow-hidden">
        <For each={filtered()}>
          {(doc, i) => (
            <div
              class="flex items-center gap-4 px-4.5 py-3.5 bg-panel border-b border-border last:border-b-0 transition-colors duration-150 hover:bg-hover anim-stagger"
              style={{ "animation-delay": `${i() * 40}ms` }}
            >
              <div class="w-34px h-34px flex-none rounded-8px bg-accent-soft text-accent flex items-center justify-center text-10.5px font-bold font-mono">
                {doc.type}
              </div>
              <div class="flex-1 min-w-0">
                <div class="text-13.5px font-semibold">{doc.name}</div>
                <div class="text-12px text-fg-muted mt-0.5">
                  {doc.pages} páginas · adicionado {doc.addedLabel}
                </div>
              </div>
              <div
                class="px-2.5 py-1 rounded-6px text-11px font-bold font-mono"
                classList={{
                  "bg-success-soft text-success": doc.status === "Indexado",
                  "bg-warning-soft text-warning": doc.status === "Processando",
                }}
              >
                {doc.status}
              </div>
              <div
                onClick={() => actions.removeDoc(doc.id)}
                class="w-7 h-7 flex-none rounded-6px flex items-center justify-center cursor-pointer text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-danger"
              >
                <div class="w-2.5 h-2.5 rounded-2px bg-current" />
              </div>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
