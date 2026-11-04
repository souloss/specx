import { defineComponent, useId } from "vue";
import SpecxGlyph, { brandProps } from "./SpecxGlyph";

export default defineComponent({
  name: "SpecxMark",
  props: brandProps,
  setup(props) {
    const titleId = `specx-mark-${useId()}`;
    return () => (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 64 64"
        width={props.size}
        height={props.size}
        class={["specx-brand", { "specx-brand--animated": props.animated }]}
        role={props.decorative ? undefined : "img"}
        aria-labelledby={props.decorative ? undefined : titleId}
        aria-hidden={props.decorative ? "true" : undefined}
        focusable="false"
      >
        {!props.decorative && <title id={titleId}>{props.label}</title>}
        <SpecxGlyph monochrome={props.monochrome} />
      </svg>
    );
  },
});
