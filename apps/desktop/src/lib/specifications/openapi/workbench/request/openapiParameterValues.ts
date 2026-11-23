import { isRecord, mergeOpenApiSchema, type OpenApiObject } from "../../contract/openapi";
import type { createOpenApiVariableResolver } from "../variables/openapiDynamicVariables";
/** Resolve expressions before schema coercion; keep strings and structured values distinct. */
export function expandOpenApiParameterValue(value: string, schema: OpenApiObject, document: OpenApiObject | null, resolver: ReturnType<typeof createOpenApiVariableResolver>, variables: Record<string, string>): string {
  const resolved = mergeOpenApiSchema(document, schema) ?? schema;
  if (resolved.type !== "object" && resolved.type !== "array" && !isRecord(resolved.properties)) return resolver.expand(value, variables);
  const expanded = resolver.expandJson(value, variables);
  let data: unknown;
  try {
    data = JSON.parse(expanded);
  } catch {
    return expanded;
  }
  function coerce(input: unknown, source: unknown, depth = 0): unknown {
    if (depth > 12) return input;
    const rule = mergeOpenApiSchema(document, source) ?? {};
    if (typeof input === "string") {
      if ((rule.type === "number" || rule.type === "integer") && input.trim() && Number.isFinite(Number(input)) && (rule.type !== "integer" || Number.isInteger(Number(input)))) return Number(input);
      if (rule.type === "boolean" && (input === "true" || input === "false")) return input === "true";
    }
    if (Array.isArray(input)) return input.map((item) => coerce(item, rule.items, depth + 1));
    if (isRecord(input)) {
      const fields = isRecord(rule.properties) ? rule.properties : {};
      return Object.fromEntries(Object.entries(input).map(([key, item]) => [key, coerce(item, fields[key] ?? rule.additionalProperties, depth + 1)]));
    }
    return input;
  }
  return JSON.stringify(coerce(data, resolved));
}
