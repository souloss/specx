import { defineComponent, useId } from "vue";
import SpecxGlyph, { brandProps } from "./SpecxGlyph";

export default defineComponent({
  name: "SpecxLogo",
  props: brandProps,
  setup(props) {
    const id = `specx-logo-${useId()}`;
    return () => (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 224 64"
        width={props.size * 3.5}
        height={props.size}
        class={["specx-brand", { "specx-brand--animated": props.animated }]}
        role={props.decorative ? undefined : "img"}
        aria-labelledby={props.decorative ? undefined : `${id}-title`}
        aria-hidden={props.decorative ? "true" : undefined}
        focusable="false"
      >
        {!props.decorative && <title id={`${id}-title`}>{props.label}</title>}
        <defs>
          <linearGradient id={`${id}-accent`} x1="189" y1="20" x2="222" y2="50" gradientUnits="userSpaceOnUse">
            <stop stop-color="#6d74ec" />
            <stop offset="1" stop-color="#9464ec" />
          </linearGradient>
        </defs>
        <SpecxGlyph monochrome={props.monochrome} />
        {/* Custom vector lettering keeps the wordmark independent of installed fonts. */}
        <g fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
          <path d="M96 26C92 22 82 23 82 28C82 34 97 31 97 38C97 44 87 46 81 41" />
          <path d="M107 24V53M107 28C112 21 125 23 125 34C125 45 112 47 107 40" />
          <path d="M135 34H153C153 20 133 20 133 34C133 46 147 47 153 41" />
          <path d="M180 27C173 20 163 25 163 34C163 44 173 48 180 41" />
        </g>
        <path d="M189 20H196L205 31L214 20H221L209 35L222 50H214L205 39L196 50H188L200 35Z" fill={props.monochrome ? "currentColor" : `url(#${id}-accent)`} />
      </svg>
    );
  },
});
