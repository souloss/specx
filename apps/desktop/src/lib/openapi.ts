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

export function isRecord(value: unknown): value is OpenApiObject {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}
