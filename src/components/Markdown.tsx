import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

/// Render a markdown string to sanitized HTML. Used for assistant replies so
/// lists, code blocks, bold/italic, etc. render properly.
export default function Markdown(props: { source: string }) {
  const html = () =>
    DOMPurify.sanitize(marked.parse(props.source ?? "", { async: false }) as string);
  return <div class="md" innerHTML={html()} />;
}
