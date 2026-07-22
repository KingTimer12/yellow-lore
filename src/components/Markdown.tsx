import { createEffect } from "solid-js";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { state, actions, type Source } from "../store";

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

  // Also match the FIRST name of multi-word entities ("Cesar" → "Cesar Magnus"),
  // since chapters often use only the given name. Skip any first name shared by
  // two entities (ambiguous) or that collides with a full entity name.
  const first = new Map<string, EntIndex[number] | null>();
  for (const e of idx) {
    const toks = e.lower.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
    if (toks.length < 2) continue;
    const t = toks[0];
    if (t.length < 3 || map.has(t)) continue;
    first.set(t, first.has(t) ? null : e);
  }
  for (const [t, e] of first) if (e && !map.has(t)) map.set(t, e);

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

/// Wrap inline `[N]` citation markers with a hoverable/clickable span, but only
/// where a source with that mark exists (so stray brackets aren't touched).
function wrapCitations(root: HTMLElement, marks: Set<number>) {
  if (marks.size === 0) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const targets: Text[] = [];
  let node: Node | null;
  while ((node = walker.nextNode())) {
    const t = node as Text;
    if (!t.nodeValue || !t.nodeValue.includes("[")) continue;
    if ((t.parentElement as HTMLElement | null)?.closest("code, pre, a, .entity-mention, .citation-ref")) continue;
    targets.push(t);
  }
  const re = /\[(\d+)\]/g;
  for (const t of targets) {
    const text = t.nodeValue!;
    re.lastIndex = 0;
    if (!re.test(text)) continue;
    re.lastIndex = 0;
    const frag = document.createDocumentFragment();
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text))) {
      const n = Number(m[1]);
      if (!marks.has(n)) continue;
      if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
      const span = document.createElement("span");
      span.className = "citation-ref";
      span.dataset.mark = String(n);
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
/// character/place names become hoverable cards; `sources` turns inline `[N]`
/// markers into hoverable/clickable citations.
export default function Markdown(props: { source: string; mentions?: boolean; sources?: Source[] }) {
  let el: HTMLDivElement | undefined;

  const sourceByMark = () => {
    const map = new Map<number, Source>();
    for (const s of props.sources ?? []) if (!map.has(s.mark)) map.set(s.mark, s);
    return map;
  };

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
    wrapCitations(el, new Set(sourceByMark().keys()));
  });

  // Event delegation: entity hover-cards (.entity-mention) + citation hover-cards
  // (.citation-ref).
  const onOver = (e: MouseEvent) => {
    const el = e.target as HTMLElement;
    if (props.mentions) {
      const men = el.closest?.(".entity-mention") as HTMLElement | null;
      if (men?.dataset.ent) {
        const c = state.characters.find((x) => x.name.toLowerCase() === men.dataset.ent);
        const p = !c ? state.places.find((x) => x.name.toLowerCase() === men.dataset.ent) : undefined;
        const r = men.getBoundingClientRect();
        if (c) actions.showMention({ name: c.name, role: c.role, summary: c.summary, traits: c.traits, kind: "character", x: r.left, y: r.bottom });
        else if (p) actions.showMention({ name: p.name, role: p.type, summary: p.summary, traits: [], kind: "place", x: r.left, y: r.bottom });
        return;
      }
    }
    const cit = el.closest?.(".citation-ref") as HTMLElement | null;
    if (cit?.dataset.mark) {
      const s = sourceByMark().get(Number(cit.dataset.mark));
      if (s) {
        const r = cit.getBoundingClientRect();
        actions.showSourcePop({ doc: s.doc, text: s.text, x: r.left, y: r.bottom });
      }
    }
  };
  const onOut = (e: MouseEvent) => {
    const el = e.target as HTMLElement;
    if (el.closest?.(".entity-mention")) actions.hideMention();
    if (el.closest?.(".citation-ref")) actions.hideSourcePop();
  };
  // Click a citation → open the full citation modal for that source's document.
  const onClick = (e: MouseEvent) => {
    const cit = (e.target as HTMLElement).closest?.(".citation-ref") as HTMLElement | null;
    if (!cit?.dataset.mark) return;
    const s = sourceByMark().get(Number(cit.dataset.mark));
    if (!s) return;
    actions.hideSourcePop();
    actions.openCitation(s.doc, [{ quote: s.quote, text: s.text }]);
  };

  return <div ref={el} class="md" onMouseOver={onOver} onMouseOut={onOut} onClick={onClick} />;
}
