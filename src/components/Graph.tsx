import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { state, actions } from "../store";

// A node in the lore graph. Characters and places share one canvas — the world
// is one web of relationships, not two separate lists.
type Kind = "character" | "place" | "unknown";
type Node = {
  id: string; // lowercased name (relations reference names)
  name: string;
  kind: Kind;
  refId?: string; // entity id, for opening the editor
  x: number;
  y: number;
  vx: number;
  vy: number;
  fx?: number; // pinned position while dragging
  fy?: number;
  deg: number;
};
type Edge = { a: string; b: string; label: string };

const COLOR: Record<Kind, string> = {
  character: "var(--accent)",
  place: "oklch(0.68 0.11 195)",
  unknown: "var(--fg-muted)",
};

// Persisted layout so re-extraction / tab switches don't reshuffle the map.
const savedPos = new Map<string, { x: number; y: number }>();

export default function Graph() {
  let svg: SVGSVGElement | undefined;
  const [size, setSize] = createSignal({ w: 800, h: 600 });
  const [frame, setFrame] = createSignal(0);
  const [view, setView] = createSignal({ scale: 1, tx: 0, ty: 0 });
  const [hover, setHover] = createSignal<string | null>(null);

  // ---- Build graph from entity data (rebuilds when data changes) ----------
  const graph = createMemo(() => {
    const nodes = new Map<string, Node>();
    const add = (name: string, kind: Kind, refId?: string) => {
      const id = name.trim().toLowerCase();
      if (!id) return;
      const prev = savedPos.get(id);
      const existing = nodes.get(id);
      if (existing) {
        if (existing.kind === "unknown" && kind !== "unknown") {
          existing.kind = kind;
          existing.refId = refId;
        }
        return;
      }
      nodes.set(id, {
        id,
        name,
        kind,
        refId,
        x: prev?.x ?? (Math.cos(nodes.size) * 120 + (nodes.size % 7) * 18 - 60),
        y: prev?.y ?? (Math.sin(nodes.size) * 120 + (nodes.size % 5) * 18 - 40),
        vx: 0,
        vy: 0,
        deg: 0,
      });
    };
    for (const c of state.characters) add(c.name, "character", c.id);
    for (const p of state.places) add(p.name, "place", p.id);
    const edges: Edge[] = [];
    for (const r of state.relations) {
      add(r.from, "unknown");
      add(r.to, "unknown");
      const a = r.from.trim().toLowerCase();
      const b = r.to.trim().toLowerCase();
      if (a && b && nodes.has(a) && nodes.has(b) && a !== b) {
        edges.push({ a, b, label: r.label });
        nodes.get(a)!.deg++;
        nodes.get(b)!.deg++;
      }
    }
    return { nodes: [...nodes.values()], edges };
  });

  const nodeById = createMemo(() => {
    const m = new Map<string, Node>();
    for (const n of graph().nodes) m.set(n.id, n);
    return m;
  });

  // Neighbors of the hovered node (for highlight / dimming).
  const neighbors = createMemo(() => {
    const h = hover();
    const set = new Set<string>();
    if (!h) return set;
    set.add(h);
    for (const e of graph().edges) {
      if (e.a === h) set.add(e.b);
      if (e.b === h) set.add(e.a);
    }
    return set;
  });

  // ---- Force simulation ----------------------------------------------------
  let alpha = 1;
  let raf = 0;
  const tick = () => {
    const { nodes, edges } = graph();
    const n = nodes.length;
    if (n > 0) {
      // Repulsion (Coulomb-ish), O(n²) — fine for tens of nodes.
      for (let i = 0; i < n; i++) {
        const a = nodes[i];
        for (let j = i + 1; j < n; j++) {
          const b = nodes[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let d2 = dx * dx + dy * dy;
          if (d2 < 0.01) { dx = Math.random() - 0.5; dy = Math.random() - 0.5; d2 = 0.01; }
          const f = (4200 / d2) * alpha;
          const d = Math.sqrt(d2);
          const fx = (dx / d) * f;
          const fy = (dy / d) * f;
          a.vx += fx; a.vy += fy;
          b.vx -= fx; b.vy -= fy;
        }
      }
      // Springs on edges toward a rest length.
      const L = 110;
      for (const e of edges) {
        const a = nodeById().get(e.a)!;
        const b = nodeById().get(e.b)!;
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const d = Math.sqrt(dx * dx + dy * dy) || 1;
        const f = ((d - L) / d) * 0.06 * alpha;
        const fx = dx * f;
        const fy = dy * f;
        a.vx += fx; a.vy += fy;
        b.vx -= fx; b.vy -= fy;
      }
      // Gravity to center + integrate.
      for (const nd of nodes) {
        nd.vx += -nd.x * 0.006 * alpha;
        nd.vy += -nd.y * 0.006 * alpha;
        if (nd.fx !== undefined) { nd.x = nd.fx; nd.y = nd.fy!; nd.vx = 0; nd.vy = 0; }
        else { nd.x += nd.vx; nd.y += nd.vy; nd.vx *= 0.82; nd.vy *= 0.82; }
      }
      alpha *= 0.9915;
      if (alpha < 0.004) alpha = 0.004; // never fully freeze; keeps drag responsive
    }
    setFrame((f) => f + 1);
    raf = requestAnimationFrame(tick);
  };

  const reheat = () => { alpha = Math.max(alpha, 0.7); };

  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (svg) setSize({ w: svg.clientWidth, h: svg.clientHeight });
    });
    if (svg) ro.observe(svg);
    raf = requestAnimationFrame(tick);
    onCleanup(() => { cancelAnimationFrame(raf); ro.disconnect(); });
  });
  onCleanup(() => {
    // Persist final positions for next mount.
    for (const nd of graph().nodes) savedPos.set(nd.id, { x: nd.x, y: nd.y });
  });

  // ---- Coordinate transform ------------------------------------------------
  const cx = () => size().w / 2 + view().tx;
  const cy = () => size().h / 2 + view().ty;
  const toGraph = (clientX: number, clientY: number) => {
    const r = svg!.getBoundingClientRect();
    const s = view().scale;
    return { x: (clientX - r.left - cx()) / s, y: (clientY - r.top - cy()) / s };
  };

  // ---- Interaction ---------------------------------------------------------
  let drag: { id: string; moved: boolean } | null = null;
  let pan: { x: number; y: number } | null = null;

  const onNodeDown = (e: PointerEvent, nd: Node) => {
    e.stopPropagation();
    (e.target as Element).setPointerCapture?.(e.pointerId);
    drag = { id: nd.id, moved: false };
    reheat();
  };
  const onDown = (e: PointerEvent) => {
    pan = { x: e.clientX - view().tx, y: e.clientY - view().ty };
  };
  const onMove = (e: PointerEvent) => {
    if (drag) {
      const g = toGraph(e.clientX, e.clientY);
      const nd = nodeById().get(drag.id);
      if (nd) { nd.fx = g.x; nd.fy = g.y; drag.moved = true; reheat(); }
    } else if (pan) {
      setView((v) => ({ ...v, tx: e.clientX - pan!.x, ty: e.clientY - pan!.y }));
    }
  };
  const onUp = () => {
    if (drag) {
      const nd = nodeById().get(drag.id);
      if (nd) { nd.fx = undefined; nd.fy = undefined; }
      if (nd && !drag.moved && nd.refId && nd.kind !== "unknown") {
        actions.openEdit(nd.kind as "character" | "place", nd.refId);
      }
      drag = null;
    }
    pan = null;
  };
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    setView((v) => ({ ...v, scale: Math.min(3, Math.max(0.3, v.scale * factor)) }));
  };

  const radius = (nd: Node) => 7 + Math.min(nd.deg, 8) * 1.6;
  const dim = (id: string) => hover() !== null && !neighbors().has(id);

  return (
    <div class="relative w-full h-full rounded-14px border border-border overflow-hidden bg-panel">
      <svg
        ref={svg}
        class="w-full h-full block touch-none select-none cursor-grab active:cursor-grabbing"
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerLeave={onUp}
        onWheel={onWheel}
      >
        <g transform={`translate(${(frame(), cx())} ${cy()}) scale(${view().scale})`}>
          {/* Edges */}
          <For each={graph().edges}>
            {(e) => {
              const a = () => nodeById().get(e.a)!;
              const b = () => nodeById().get(e.b)!;
              const active = () => hover() === e.a || hover() === e.b;
              return (
                <>
                  <line
                    x1={(frame(), a().x)} y1={a().y}
                    x2={(frame(), b().x)} y2={b().y}
                    stroke="var(--border)"
                    stroke-width={active() ? 1.8 : 1}
                    style={{ stroke: active() ? "var(--accent)" : "var(--border)", opacity: hover() && !active() ? 0.25 : 0.7 }}
                  />
                  <Show when={active() && e.label}>
                    <text
                      x={(frame(), (a().x + b().x) / 2)}
                      y={(a().y + b().y) / 2 - 3}
                      text-anchor="middle"
                      class="font-sans"
                      style={{ "font-size": "9px", fill: "var(--fg-muted)" }}
                    >
                      {e.label}
                    </text>
                  </Show>
                </>
              );
            }}
          </For>

          {/* Nodes */}
          <For each={graph().nodes}>
            {(nd) => (
              <g
                transform={`translate(${(frame(), nd.x)} ${nd.y})`}
                style={{ cursor: nd.kind === "unknown" ? "grab" : "pointer", opacity: dim(nd.id) ? 0.3 : 1 }}
                onPointerDown={(e) => onNodeDown(e, nd)}
                onMouseEnter={() => setHover(nd.id)}
                onMouseLeave={() => setHover(null)}
              >
                <circle
                  r={radius(nd)}
                  fill={COLOR[nd.kind]}
                  stroke="var(--panel)"
                  stroke-width={2}
                  style={{ filter: hover() === nd.id ? "brightness(1.15)" : "none" }}
                />
                <text
                  y={radius(nd) + 12}
                  text-anchor="middle"
                  class="font-serif"
                  style={{ "font-size": "11.5px", "font-weight": 600, fill: "var(--fg)", "paint-order": "stroke", stroke: "var(--panel)", "stroke-width": "3px" }}
                >
                  {nd.name}
                </text>
              </g>
            )}
          </For>
        </g>
      </svg>

      {/* Legend */}
      <div class="absolute left-3 bottom-3 flex items-center gap-3.5 px-3 py-2 rounded-9px bg-bg/80 border border-border backdrop-blur text-11.5px">
        <div class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full" style={{ background: COLOR.character }} /> Personagem</div>
        <div class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full" style={{ background: COLOR.place }} /> Lugar</div>
      </div>
      <div class="absolute right-3 bottom-3 text-10.5px text-fg-muted">
        arraste para mover · rolar para zoom · clique num nó para editar
      </div>

      <Show when={graph().nodes.length === 0}>
        <div class="absolute inset-0 flex items-center justify-center text-13px text-fg-muted">
          Nenhuma entidade ainda — extraia da base para montar o grafo.
        </div>
      </Show>
    </div>
  );
}
