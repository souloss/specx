import { defineComponent, useId } from "vue";
import "./brand.css";

export const brandProps = {
  size: { type: Number, default: 32 },
  animated: Boolean,
  monochrome: Boolean,
  decorative: Boolean,
  label: { type: String, default: "specX" },
};

// Shared artwork in a 64 × 64 coordinate system. Each instance owns its SVG IDs.
export default defineComponent({
  name: "SpecxGlyph",
  props: { monochrome: Boolean },
  setup(props) {
    const id = `specx-glyph-${useId()}`;
    const tile = "M19 4H45L60 19V45L45 60H19L4 45V19Z";
    const slash = "M37 17H48L27 47H16Z";
    const fold = "M16 17H27L48 47H37Z";

    return () => (
      <g>
        <defs>
          <linearGradient id={`${id}-surface`} x1="4" y1="4" x2="60" y2="60" gradientUnits="userSpaceOnUse">
            <stop stop-color="#242c48" />
            <stop offset="1" stop-color="#0b1020" />
          </linearGradient>
          <linearGradient id={`${id}-edge`} x1="4" y1="4" x2="60" y2="60" gradientUnits="userSpaceOnUse">
            <stop stop-color="#a5b4fc" />
            <stop offset="0.5" stop-color="#626a9c" stop-opacity="0.2" />
            <stop offset="1" stop-color="#5eead4" stop-opacity="0.6" />
          </linearGradient>
          <linearGradient id={`${id}-ice`} x1="43" y1="17" x2="21" y2="47" gradientUnits="userSpaceOnUse">
            <stop stop-color="#c1fff4" />
            <stop offset="0.45" stop-color="#5eead4" />
            <stop offset="1" stop-color="#38bdf8" />
          </linearGradient>
          <linearGradient id={`${id}-violet`} x1="16" y1="17" x2="48" y2="47" gradientUnits="userSpaceOnUse">
            <stop stop-color="#a5b4fc" />
            <stop offset="0.5" stop-color="#818cf8" />
            <stop offset="1" stop-color="#8b5cf6" />
          </linearGradient>
          <mask id={`${id}-crossing`} maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
            <rect width="64" height="64" fill="white" />
            <path d={fold} fill="black" stroke="black" stroke-width="3" />
          </mask>
        </defs>
        <path d={tile} fill={props.monochrome ? "none" : `url(#${id}-surface)`} stroke={props.monochrome ? "currentColor" : `url(#${id}-edge)`} stroke-opacity={props.monochrome ? 0.3 : 1} />
        <path class="specx-glyph__trace" d={tile} fill="none" stroke={props.monochrome ? "currentColor" : "#99f6e4"} stroke-width="1.5" pathLength="236" />
        <g class="specx-glyph__ribbons">
          <path d={slash} fill={props.monochrome ? "currentColor" : `url(#${id}-ice)`} mask={`url(#${id}-crossing)`} />
          <path d={fold} fill={props.monochrome ? "currentColor" : `url(#${id}-violet)`} />
          {!props.monochrome && <path d="M17 17.5H26.5L47 46.5" fill="none" stroke="white" stroke-opacity="0.35" />}
        </g>
        <path d="M17 10H25M54 39V45L45 54" fill="none" stroke={props.monochrome ? "currentColor" : "#a5b4fc"} stroke-opacity="0.25" stroke-linecap="round" />
      </g>
    );
  },
});
