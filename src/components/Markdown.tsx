import { createEffect } from "solid-js";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { state, actions } from "../store";

marked.setOptions({ gfm: true, breaks: true });

type EntIndex = { lower: string; name: string; role: string; summary: string; traits: string[]; kind: "character" | "place" }[];

/// Escape a string for safe use inside a RegExp.
function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/// Build a lookup + a single alternation regex (longest names first so "Cesar
/// Magnus" wins over "Cesar") for the known entities. Null when there's nothing
/// to match.
function buildIndex(): { map: Map<string, EntIndex[number]>; re: RegExp } | null {
  const idx: EntIndex = [];
  for (const c of state.characters) {
    if (c.name.trim()) idx.push({ lower: c.name.toLowerCase(), name: c.name, role: c.role, summary: c.summary, traits: c.traits, kind: "character" });
  }
  for (const p of state.places) {
    if (p.name.trim()) idx.push({ lower: p.name.toLowerCase(), name: p.name, role: p.type, summary: p.summary, traits: [], kind: "place" });
  }
  if (idx.length === 0) return null;
  const map = new Map<string, EntIndex[number]>();
  for (const e of idx) if (!map.has(e.lower)) map.set(e.lower, e);
  const names = [...map.keys()].sort((a, b) => b.length - a.length).map(escapeRe);
  // Unicode-aware "word" boundaries via lookarounds so accented names match.
  const re = new RegExp(`(?<!\\p{L})(${names.join("|")})(?!\\p{L})`, "giu");
  return { map, re };
}

/// Wrap occurrences of known entity names (in text nodes only — never inside
/// code, links or existing mentions) with a hoverable span.
function wrapMentions(root: HTMLElement) {
  const built = buildIndex();
  if (!built) return;
  const { map, re } = built;

  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const targets: Text[] = [];
  let node: Node | null;
  while ((node = walker.nextNode())) {
    const t = node as Text;
    if (!t.nodeValue || !t.nodeValue.trim()) continue;
    if ((t.parentElement as HTMLElement | null)?.closest("code, pre, a, .entity-mention")) continue;
    targets.push(t);
  }

  for (const t of targets) {
    const text = t.nodeValue!;
    re.lastIndex = 0;
    if (!re.test(text)) continue;
    re.lastIndex = 0;
    const frag = document.createDocumentFragment();
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text))) {
      const ent = map.get(m[0].toLowerCase());
      if (!ent) continue;
      if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
      const span = document.createElement("span");
      span.className = "entity-mention";
      span.dataset.ent = ent.lower;
      span.textContent = m[0];
      frag.appendChild(span);
      last = m.index + m[0].length;
    }
    if (last > 0) {
      if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
      t.parentNode?.replaceChild(frag, t);
    }
  }
}

/// Render a markdown string to sanitized HTML. Used for assistant replies so
/// lists, code blocks, bold/italic, etc. render properly. With `mentions`, known
/// character/place names become hoverable cards.
export default function Markdown(props: { source: string; mentions?: boolean }) {
  let el: HTMLDivElement | undefined;

  createEffect(() => {
    const html = DOMPurify.sanitize(marked.parse(props.source ?? "", { async: false }) as string);
    if (!el) return;
    el.innerHTML = html;
    // Re-read entities so mentions appear once extraction populates them.
    if (props.mentions) {
      state.characters;
      state.places;
      wrapMentions(el);
    }
  });

  // Event delegation: show/hide the entity hover-card for `.entity-mention`.
  const onOver = (e: MouseEvent) => {
    if (!props.mentions) return;
    const target = (e.target as HTMLElement).closest?.(".entity-mention") as HTMLElement | null;
    if (!target?.dataset.ent) return;
    const c = state.characters.find((x) => x.name.toLowerCase() === target.dataset.ent);
    const p = !c ? state.places.find((x) => x.name.toLowerCase() === target.dataset.ent) : undefined;
    const r = target.getBoundingClientRect();
    if (c) actions.showMention({ name: c.name, role: c.role, summary: c.summary, traits: c.traits, kind: "character", x: r.left, y: r.bottom });
    else if (p) actions.showMention({ name: p.name, role: p.type, summary: p.summary, traits: [], kind: "place", x: r.left, y: r.bottom });
  };
  const onOut = (e: MouseEvent) => {
    if (!props.mentions) return;
    if ((e.target as HTMLElement).closest?.(".entity-mention")) actions.hideMention();
  };

  return <div ref={el} class="md" onMouseOver={onOver} onMouseOut={onOut} />;
}
