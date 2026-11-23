import { stringify } from "yaml";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { getOpenApiOperations, isRecord, type OpenApiObject, type OpenApiOperationEntry } from "../contract/openapi";
import { getOperationParameters } from "../contract/openapiParameters";
import { operationMarkdown } from "./openapiDocumentation";
import { getOpenApiModels, type OpenApiModel } from "../contract/openapiModels";
import { localOpenApiDefinitions, openApiPointerParts, openApiPointerValue, visitOpenApiReferences } from "../contract/openapiReferences";

/** Portable page export: retain the local reference closure, including cycles,
 * without unrelated operations. No workspace lookup or network resolution. */
export function operationOpenApiDocument(document: OpenApiObject | null, entry: OpenApiOperationEntry): OpenApiObject {
  const operation = JSON.parse(JSON.stringify(entry.operation)) as OpenApiObject;
  const subset: OpenApiObject = {
    openapi: document?.openapi ?? "3.0.3",
    info: document?.info ?? { title: "API", version: "1" },
    paths: { [entry.path]: { parameters: getOperationParameters(document, entry), [entry.method]: operation } },
  };
  if (document?.["x-specx-public-environments"] !== undefined) subset["x-specx-public-environments"] = document["x-specx-public-environments"];
  const security = entry.operation.security ?? document?.security;
  if (security !== undefined) subset.security = security;
  if (document?.servers !== undefined) subset.servers = document.servers;
  const pathItem = isRecord(document?.paths) ? document.paths[entry.path] : undefined;
  if (isRecord(pathItem) && pathItem.servers !== undefined) {
    (subset.paths as OpenApiObject)[entry.path] = { ...((subset.paths as OpenApiObject)[entry.path] as OpenApiObject), servers: pathItem.servers };
  }
  const schemes = isRecord(document?.components) && isRecord(document.components.securitySchemes) ? document.components.securitySchemes : {};
  if (Array.isArray(security)) {
    const used = Object.fromEntries(security.filter(isRecord).flatMap((item) => Object.keys(item).flatMap((name) => (schemes[name] !== undefined ? [[name, schemes[name]]] : []))));
    if (Object.keys(used).length) subset.components = { securitySchemes: used };
  }
  const visited = new Set<string>();
  function include(reference: string) {
    const parts = openApiPointerParts(reference);
    if (!parts?.length || visited.has(reference) || !document) return;
    visited.add(reference);
    const target = openApiPointerValue(document, reference);
    if (target === undefined) return;
    let parent = subset;
    for (const key of parts.slice(0, -1)) {
      if (!Object.hasOwn(parent, key) || !isRecord(parent[key])) Object.defineProperty(parent, key, { value: {}, enumerable: true, writable: true, configurable: true });
      parent = parent[key] as OpenApiObject;
    }
    Object.defineProperty(parent, parts.at(-1)!, { value: JSON.parse(JSON.stringify(target)), enumerable: true, writable: true, configurable: true });
    visitOpenApiReferences(target, include);
  }
  visitOpenApiReferences(subset, include);
  return JSON.parse(JSON.stringify(subset)) as OpenApiObject;
}

export function modelMarkdown(model: OpenApiModel, document: OpenApiObject | null = null): string {
  const definitions = localOpenApiDefinitions(document, model.schema);
  const references = Object.keys(definitions).length ? `\n\n## Referenced definitions\n\n\`\`\`json\n${JSON.stringify(definitions, null, 2)}\n\`\`\`` : "";
  return `# ${model.name}\n\n${model.description}\n\n\`\`\`json\n${JSON.stringify(model.schema, null, 2)}\n\`\`\`${references}`;
}
export function documentMarkdown(document: OpenApiObject, title: string): string {
  const info = isRecord(document.info) ? document.info : {};
  const operations = getOpenApiOperations(JSON.stringify(document));
  return [`# ${title}`, String(info.description ?? ""), ...operations.map((entry) => operationMarkdown(document, entry)), ...getOpenApiModels(document).map((model) => modelMarkdown(model, document))].filter(Boolean).join("\n\n---\n\n");
}

export function documentationIndex(document: OpenApiObject, title: string, baseUrl: string): string {
  const url = baseUrl.split("#")[0]!;
  const info = isRecord(document.info) ? document.info : {};
  const link = (key: string) => `${url}#${encodeURIComponent(key)}`;
  return [
    `# ${title}`,
    `> ${String(info.description ?? "OpenAPI reference").replaceAll("\n", " ")}`,
    "## API",
    ...getOpenApiOperations(JSON.stringify(document)).map((entry) => `- [${entry.method.toUpperCase()} ${entry.path}](${link(entry.identityKey)}): ${String(entry.operation.summary ?? entry.operation.operationId ?? "").replaceAll("\n", " ")}`),
    "## Models",
    ...getOpenApiModels(document).map((model) => `- [${model.name}](${link(`model:${model.reference}`)}): ${model.description.replaceAll("\n", " ")}`),
  ].join("\n\n");
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
}
export function documentationHtml(markdown: string, title: string): string {
  const content = DOMPurify.sanitize(marked.parse(markdown, { async: false }));
  return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><title>${escapeHtml(title)}</title><style>:root{color-scheme:light dark}body{max-width:960px;margin:0 auto;padding:32px;font:15px/1.7 system-ui,sans-serif;overflow-wrap:anywhere}pre{overflow:auto;padding:18px;border:1px solid #8885;border-radius:8px}code{font-family:ui-monospace,monospace;font-size:13px}table{border-collapse:collapse;display:block;overflow:auto}td,th{border:1px solid #8885;padding:8px}img{max-width:100%}hr{border:0;border-top:1px solid #8885;margin:40px 0}@media print{pre{white-space:pre-wrap}}</style></head><body>${content}</body></html>`;
}

export function openApiYaml(document: OpenApiObject): string {
  return stringify(document, { lineWidth: 0 });
}
