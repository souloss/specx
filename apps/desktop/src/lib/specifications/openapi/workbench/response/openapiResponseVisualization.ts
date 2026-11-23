import Handlebars from "handlebars";
/** Templates receive data only. Prototype access and helpers invoking host code are unavailable. */
export function renderResponseVisualization(template: string, data: unknown): string {
  return Handlebars.compile(template, { strict: false })(data, { allowProtoMethodsByDefault: false, allowProtoPropertiesByDefault: false });
}

/** A self-contained post-response example; no scripts or external assets run in the rendered frame. */
export const RESPONSE_VISUALIZATION_DEMO = `const payload = pm.response.json();
const rows = (Array.isArray(payload) ? payload : [payload]).map(item => ({
  fields: Object.entries(item && typeof item === "object" ? item : { value: item })
    .map(([key, value]) => ({ key, value: typeof value === "object" ? JSON.stringify(value) : String(value) }))
}));
pm.visualizer.set(\`<style>
  :root { color-scheme: light dark; font: 14px/1.6 system-ui; }
  body { margin: 16px; } dl { margin: 0 0 16px; border: 1px solid #8885; border-radius: 8px; overflow: hidden; }
  .field { display: grid; grid-template-columns: minmax(80px, 1fr) minmax(0, 3fr); border-bottom: 1px solid #8883; }
  .field:last-child { border: 0; } dt, dd { margin: 0; padding: 10px 12px; overflow-wrap: anywhere; }
  dt { font-weight: 600; background: #8881; } dd { white-space: pre-wrap; }
</style>{{#each rows}}<dl>{{#each fields}}<div class="field"><dt>{{key}}</dt><dd>{{value}}</dd></div>{{/each}}</dl>{{/each}}\`, { rows });`;
