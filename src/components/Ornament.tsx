/// A rubricated section rule: a hairline broken by a small fleuron, echoing the
/// way a manuscript marks the start of a new passage.
export default function Ornament(props: { class?: string }) {
  return (
    <div class={`ornament ${props.class ?? ""}`}>
      <span class="text-accent text-11px leading-none select-none">&#10087;</span>
    </div>
  );
}
