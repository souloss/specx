import { isRecord, type OpenApiObject, type OpenApiOperationEntry } from "../contract/openapi";
import { getOperationParameters } from "../contract/openapiParameters";
import type { OpenApiModel } from "../contract/openapiModels";
import { openApiPointerValue } from "../contract/openapiReferences";

export type OpenApiSearchIndex = { operations: Map<string, string>; models: Map<string, string> };

/** Index contract text once, outside the input/render path. Reference cycles and
 * example payloads must not make field search unbounded or index arbitrary data. */
export function buildOpenApiSearchIndex(document: OpenApiObject | null, operations: OpenApiOperationEntry[], models: OpenApiModel[]): OpenApiSearchIndex {
  const referenceText = new Map<string, string>();
  function contractText(source: unknown): string {
    const text: string[] = [];
    const visited = new Set<string>();
    let remaining = 20_000;
    function visit(value: unknown, depth = 0) {
      if (remaining-- <= 0 || depth > 64) return;
      if (Array.isArray(value)) {
        for (const child of value) visit(child, depth + 1);
      } else if (isRecord(value)) {
        for (const [key, child] of Object.entries(value)) {
          if (["example", "examples", "default", "enum", "const", "value"].includes(key) || key.startsWith("x-")) continue;
          if (["properties", "patternProperties", "$defs", "definitions"].includes(key) && isRecord(child)) {
            // A field named example/value/x-id is still contract data. Only
            // skip payload keywords in a schema, never keys in its field map.
            for (const [name, schema] of Object.entries(child)) {
              text.push(name);
              visit(schema, depth + 1);
            }
          } else if (key === "$ref" && typeof child === "string") {
            text.push(child);
            if (!document || !child.startsWith("#/") || visited.has(child)) continue;
            visited.add(child);
            const cached = referenceText.get(child);
            if (cached !== undefined) text.push(cached);
            else visit(openApiPointerValue(document, child), depth + 1);
          } else {
            text.push(key);
            if (typeof child === "string") text.push(child);
            else visit(child, depth + 1);
          }
        }
      }
    }
    visit(source);
    return text.join(" ").toLowerCase();
  }
  const modelEntries = models.map((model) => {
    const text = `${model.name} ${model.description} ${model.reference} ${contractText(model.schema)}`.toLowerCase();
    referenceText.set(model.reference, text);
    return [model.reference, text] as const;
  });
  return {
    models: new Map(modelEntries),
    operations: new Map(operations.map((entry) => [entry.identityKey, `${entry.method} ${entry.path} ${entry.tags.join(" ")} ${contractText({ ...entry.operation, parameters: getOperationParameters(document, entry) })}`.toLowerCase()])),
  };
}

export function matchesOpenApiSearch(text: string, query: string): boolean {
  return query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .every((term) => text.includes(term));
}
