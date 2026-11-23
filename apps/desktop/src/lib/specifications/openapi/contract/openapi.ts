import { parse, stringify } from "yaml";

export const OPENAPI_METHODS = ["get", "put", "post", "delete", "options", "head", "patch", "trace"] as const;
export type OpenApiMethod = (typeof OPENAPI_METHODS)[number];
export type OpenApiObject = Record<string, unknown>;
export type OpenApiOperationEntry = {
  method: OpenApiMethod;
  path: string;
  operation: OpenApiObject;
  identityKey: string;
  tags: string[];
};
export type OpenApiMediaExample = { name: string; label: string; description: string; value: unknown };

export function openApiMethodClass(method: string): string {
  const classes: Record<string, string> = {
    GET: "bg-emerald-600 text-white",
    POST: "bg-blue-600 text-white",
    PUT: "bg-amber-500 text-black",
    PATCH: "bg-orange-600 text-white",
    DELETE: "bg-rose-600 text-white",
    OPTIONS: "bg-slate-600 text-white",
    HEAD: "bg-cyan-700 text-white",
    TRACE: "bg-violet-700 text-white",
  };
  return classes[method.toUpperCase()] ?? "bg-muted text-foreground";
}

export function parseOpenApiContent(content: string): OpenApiObject | null {
  try {
    const parsed: unknown = content.trimStart().startsWith("{") ? JSON.parse(content) : parse(content);
    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function formatOpenApiContent(content: string, format: "json" | "yaml"): string {
  const parsed = parseOpenApiContent(content);
  if (!parsed) throw new Error("invalid_document");
  return format === "json" ? `${JSON.stringify(parsed, null, 2)}\n` : stringify(parsed, { lineWidth: 0 });
}

export function getOpenApiOperations(content: string): OpenApiOperationEntry[] {
  const document = parseOpenApiContent(content);
  if (!document || !isRecord(document.paths)) return [];
  const result: OpenApiOperationEntry[] = [];
  for (const [path, pathValue] of Object.entries(document.paths)) {
    if (!isRecord(pathValue)) continue;
    for (const method of OPENAPI_METHODS) {
      const operationValue = pathValue[method];
      if (!isRecord(operationValue)) continue;
      const tags = Array.isArray(operationValue.tags) ? operationValue.tags.filter((tag): tag is string => typeof tag === "string") : [];
      result.push({ method, path, operation: operationValue, identityKey: `${method.toUpperCase()}:${path}`, tags: tags.length ? tags : ["default"] });
    }
  }
  return result.sort((a, b) => a.path.localeCompare(b.path) || a.method.localeCompare(b.method));
}

type OpenApiServerScope = Pick<OpenApiOperationEntry, "operation" | "path">;

/** Operation and Path Item server definitions override the document defaults. */
export function getOpenApiServers(document: OpenApiObject | null, entry?: OpenApiServerScope): Array<OpenApiObject & { url: string }> {
  const paths = isRecord(document?.paths) ? document.paths : null;
  const pathValue = entry && paths ? paths[entry.path] : null;
  const pathItem = isRecord(pathValue) ? pathValue : null;
  for (const scope of [entry?.operation, pathItem, document]) {
    if (!Array.isArray(scope?.servers)) continue;
    return scope.servers.filter((server): server is OpenApiObject & { url: string } => isRecord(server) && typeof server.url === "string");
  }
  return [];
}

export function expandOpenApiServerUrl(document: OpenApiObject | null, url: string, values: Record<string, string> = {}, entry?: OpenApiServerScope): string {
  const server = getOpenApiServers(document, entry).find((candidate) => candidate.url === url);
  const variables = server && isRecord(server.variables) ? server.variables : null;
  return url.replace(/\{([^}]+)\}/g, (match, key: string) => {
    const variable = variables && isRecord(variables[key]) ? variables[key] : null;
    const value = values[key] ?? (typeof variable?.default === "string" ? variable.default : undefined);
    return value ?? match;
  });
}

export function isRecord(value: unknown): value is OpenApiObject {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

export function resolveOpenApiRef(document: OpenApiObject | null, value: unknown): OpenApiObject | null {
  const schema = isRecord(value) ? value : null;
  if (!schema || !document) return schema;
  let current: OpenApiObject = schema;
  const seen = new Set<string>();
  for (let depth = 0; depth < 32; depth += 1) {
    if (typeof current.$ref !== "string" || !current.$ref.startsWith("#/")) return current;
    if (seen.has(current.$ref)) return current;
    seen.add(current.$ref);
    const target = resolveLocalPointer(document, current.$ref);
    if (!isRecord(target)) return current;
    current = target;
  }
  return current;
}

function resolveLocalPointer(document: OpenApiObject | null, reference: string): unknown {
  let target: unknown = document;
  for (const part of reference.slice(2).split("/")) {
    let key: string;
    try {
      key = decodeURIComponent(part).replaceAll("~1", "/").replaceAll("~0", "~");
    } catch {
      return undefined;
    }
    if (Array.isArray(target) && /^(?:0|[1-9]\d*)$/.test(key)) target = target[Number(key)];
    else if (isRecord(target) && Object.hasOwn(target, key)) target = target[key];
    else return undefined;
  }
  return target;
}

/** Materialize only an edited schema, retaining 3.1 reference siblings as
 * intersecting rules. Untouched references remain in the original document.
 */
export function materializeOpenApiSchema(document: OpenApiObject | null, value: OpenApiObject): OpenApiObject {
  const modern = typeof document?.openapi === "string" && document.openapi.startsWith("3.1");
  const siblings: OpenApiObject[] = [];
  const seen = new Set<string>();
  let current = value;
  for (let depth = 0; depth < 32; depth += 1) {
    if (typeof current.$ref !== "string") return siblings.length ? { allOf: [current, ...siblings] } : current;
    if (!current.$ref.startsWith("#/") || seen.has(current.$ref)) return value;
    seen.add(current.$ref);
    if (modern) {
      const sibling = Object.fromEntries(Object.entries(current).filter(([key]) => key !== "$ref"));
      if (Object.keys(sibling).length) siblings.push(sibling);
    }
    const raw = resolveLocalPointer(document, current.$ref);
    if (!isRecord(raw)) return value;
    current = raw;
  }
  return value;
}

/**
 * Resolve a schema for presentation while retaining sibling metadata and
 * flattening object composition. OpenAPI uses allOf heavily for response
 * envelopes; leaving it as a list of opaque refs makes the useful fields
 * invisible in a documentation view.
 */
export function mergeOpenApiSchema(document: OpenApiObject | null, value: unknown, seen = new Set<string>()): OpenApiObject | null {
  const source = isRecord(value) ? value : null;
  if (!source) return null;
  const ref = typeof source.$ref === "string" ? source.$ref : null;
  if (ref && seen.has(ref)) return resolveOpenApiRef(document, source);
  const nextSeen = new Set(seen);
  if (ref) nextSeen.add(ref);
  const resolved = resolveOpenApiRef(document, source);
  if (!resolved) return null;
  const merged: OpenApiObject = { ...resolved };
  for (const [key, nested] of Object.entries(source)) if (key !== "$ref") merged[key] = nested;

  const allOf = Array.isArray(merged.allOf) ? merged.allOf : [];
  if (!allOf.length) return merged;
  delete merged.allOf;
  const properties: OpenApiObject = isRecord(merged.properties) ? { ...merged.properties } : {};
  const required = new Set<string>(Array.isArray(merged.required) ? merged.required.filter((item): item is string => typeof item === "string") : []);
  for (const branch of allOf) {
    const resolvedBranch = mergeOpenApiSchema(document, branch, nextSeen);
    if (!resolvedBranch) continue;
    if (merged.type === undefined && resolvedBranch.type !== undefined) merged.type = resolvedBranch.type;
    if (merged.description === undefined && resolvedBranch.description !== undefined) merged.description = resolvedBranch.description;
    if (isRecord(resolvedBranch.properties)) Object.assign(properties, resolvedBranch.properties);
    if (Array.isArray(resolvedBranch.required)) for (const item of resolvedBranch.required) if (typeof item === "string") required.add(item);
  }
  if (Object.keys(properties).length) merged.properties = properties;
  if (required.size) merged.required = [...required];
  return merged;
}

export function openApiSchemaName(value: unknown): string | null {
  if (!isRecord(value) || typeof value.$ref !== "string") return null;
  const parts = value.$ref.split("/");
  return parts.length ? (parts.at(-1) ?? "").replaceAll("~1", "/").replaceAll("~0", "~") : null;
}

export function generateOpenApiExample(document: OpenApiObject | null, value: unknown, depth = 0, seen = new Set<string>()): unknown {
  if (depth > 12) return null;
  const original = isRecord(value) ? value : {};
  const ref = typeof original.$ref === "string" ? original.$ref : null;
  if (ref && seen.has(ref)) return null;
  const nextSeen = new Set(seen);
  if (ref) nextSeen.add(ref);
  const schema = resolveOpenApiRef(document, value);
  if (!schema) return null;
  if (schema.example !== undefined) return schema.example;
  if (schema.default !== undefined) return schema.default;
  if (Array.isArray(schema.enum) && schema.enum.length) return schema.enum[0];
  for (const key of ["oneOf", "anyOf", "allOf"] as const) {
    if (Array.isArray(schema[key]) && schema[key].length) {
      if (key === "allOf") {
        return schema[key].reduce<Record<string, unknown>>((result, item) => {
          const generated = generateOpenApiExample(document, item, depth + 1, nextSeen);
          return isRecord(generated) ? { ...result, ...generated } : result;
        }, {});
      }
      return generateOpenApiExample(document, schema[key][0], depth + 1, nextSeen);
    }
  }
  if (schema.type === "object" || schema.properties) {
    const result: Record<string, unknown> = {};
    if (isRecord(schema.properties)) for (const [key, property] of Object.entries(schema.properties)) result[key] = generateOpenApiExample(document, property, depth + 1, nextSeen);
    return result;
  }
  if (schema.type === "array" || schema.items) return [generateOpenApiExample(document, schema.items, depth + 1, nextSeen)];
  if (schema.type === "integer" || schema.type === "number") return 0;
  if (schema.type === "boolean") return true;
  if (schema.format === "date") return "2026-01-01";
  if (schema.format === "date-time") return "2026-01-01T00:00:00Z";
  return "string";
}

export function getOpenApiMediaExamples(document: OpenApiObject | null, value: unknown): OpenApiMediaExample[] {
  const media = resolveOpenApiRef(document, value);
  if (!media) return [];
  const result: OpenApiMediaExample[] = [];
  if (media.example !== undefined) result.push({ name: "default", label: "Example", description: "", value: media.example });
  if (isRecord(media.examples)) {
    for (const [name, rawExample] of Object.entries(media.examples)) {
      const example = resolveOpenApiRef(document, rawExample);
      if (!example || !("value" in example)) continue;
      result.push({
        name,
        label: typeof example.summary === "string" && example.summary.trim() ? example.summary : name,
        description: typeof example.description === "string" ? example.description : "",
        value: example.value,
      });
    }
  }
  return result;
}

export function formatOpenApiExample(value: unknown, mediaType: string): string {
  if (mediaType.includes("yaml") || mediaType.includes("yml")) return stringify(value, { lineWidth: 0 });
  if (mediaType.includes("json") || typeof value !== "string") return `${JSON.stringify(value, null, 2)}\n`;
  return value;
}
