import { isRecord, type OpenApiObject } from "./openapi";

export function openApiPointer(parts: string[]): string {
  return `#/${parts.map((part) => encodeURIComponent(part.replaceAll("~", "~0").replaceAll("/", "~1"))).join("/")}`;
}

export function openApiPointerParts(reference: string): string[] | null {
  if (!reference.startsWith("#")) return null;
  try {
    const fragment = decodeURIComponent(reference.slice(1));
    if (!fragment.startsWith("/")) return null;
    return fragment
      .slice(1)
      .split("/")
      .map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"));
  } catch {
    return null;
  }
}

export function openApiPointerValue(document: OpenApiObject, reference: string): unknown {
  const parts = openApiPointerParts(reference);
  if (!parts) return undefined;
  let current: unknown = document;
  for (const key of parts) {
    if ((isRecord(current) || Array.isArray(current)) && Object.hasOwn(current, key)) current = (current as Record<string, unknown>)[key];
    else return undefined;
  }
  return current;
}

/** Replace a Schema reference without discarding meaningful OpenAPI 3.1 siblings. */
export function replaceOpenApiSchemaReference(document: OpenApiObject | null, schema: OpenApiObject, reference: string): OpenApiObject {
  const currentReference = typeof schema.$ref === "string";
  const modern = typeof document?.openapi === "string" && document.openapi.startsWith("3.1");
  const siblings = currentReference ? Object.fromEntries(Object.entries(schema).filter(([key]) => modern && key !== "$ref")) : {};
  return { ...siblings, $ref: reference };
}

const entryMaps = new Set(["properties", "patternProperties", "schemas", "$defs", "definitions", "dependentSchemas", "responses", "parameters", "headers", "links", "callbacks", "requestBodies", "securitySchemes", "content", "examples", "paths", "webhooks"]);

/** Visit contract references, never values embedded in examples, defaults or extensions.
 * Map keys are user names: a property named `default` or `x-data` is still a schema.
 */
export function visitOpenApiReferences(value: unknown, visit: (reference: string, path: string[]) => string | void, trail: string[] = []): void {
  if (Array.isArray(value)) {
    value.forEach((item, index) => visitOpenApiReferences(item, visit, [...trail, String(index)]));
    return;
  }
  if (!isRecord(value)) return;
  const namedEntry = entryMaps.has(trail.at(-1) ?? "");
  for (const [key, child] of Object.entries(value)) {
    const path = [...trail, key];
    if (!namedEntry && typeof child === "string" && (["$ref", "$dynamicRef", "operationRef"].includes(key) || (trail.at(-1) === "mapping" && trail.at(-2) === "discriminator"))) {
      const replacement = visit(child, path);
      if (replacement !== undefined) value[key] = replacement;
    } else if (!namedEntry && (["example", "default", "enum", "const"].includes(key) || key.startsWith("x-") || (key === "value" && trail.at(-2) === "examples") || (key === "examples" && Array.isArray(child)))) continue;
    else visitOpenApiReferences(child, visit, path);
  }
}

/** Collect a portable local reference closure without resolving external URLs. */
export function localOpenApiDefinitions(document: OpenApiObject | null, value: unknown): Record<string, unknown> {
  const definitions: Record<string, unknown> = {};
  const seen = new Set<string>();
  function scan(source: unknown) {
    visitOpenApiReferences(source, (reference) => {
      if (!document || seen.has(reference) || !openApiPointerParts(reference)) return;
      seen.add(reference);
      const target = openApiPointerValue(document, reference);
      if (target === undefined) return;
      Object.defineProperty(definitions, reference, { value: target, enumerable: true });
      scan(target);
    });
  }
  scan(value);
  return definitions;
}
