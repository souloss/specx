import { isRecord, type OpenApiObject } from "../contract/openapi";

type ObservedType = "null" | "array" | "object" | "string" | "number" | "boolean";

function observedType(value: unknown): ObservedType {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (isRecord(value)) return "object";
  return typeof value === "number" ? "number" : typeof value === "boolean" ? "boolean" : "string";
}

/** The input is a parsed, losslessly representable response example. Infer shape, not validation rules. */
export function inferResponseSchema(value: unknown, version: "3.0" | "3.1"): OpenApiObject | null {
  // The inferred result must remain practical to review in the visual editor.
  let budget = 160;
  function infer(samples: unknown[], depth: number): OpenApiObject {
    if (depth > 8 || --budget < 0) throw new RangeError("response_schema_limit");
    const grouped = new Map<ObservedType, unknown[]>();
    for (const sample of samples) {
      const type = observedType(sample);
      const group = grouped.get(type);
      if (group) group.push(sample);
      else grouped.set(type, [sample]);
    }
    const nullable = grouped.delete("null");
    const variants: OpenApiObject[] = [];
    for (const [type, group] of grouped) {
      if (type === "object") {
        const properties = new Map<string, unknown[]>();
        for (const sample of group) {
          if (!isRecord(sample)) continue;
          for (const [key, value] of Object.entries(sample)) {
            const values = properties.get(key);
            if (values) values.push(value);
            else properties.set(key, [value]);
          }
        }
        variants.push({ type, properties: Object.fromEntries([...properties].map(([key, values]) => [key, infer(values, depth + 1)])) });
      } else if (type === "array") {
        const items = group.flat();
        variants.push({ type, items: items.length ? infer(items, depth + 1) : {} });
      } else variants.push({ type: type === "number" && group.every(Number.isInteger) ? "integer" : type });
    }
    if (!variants.length) return nullable && version === "3.1" ? { type: "null" } : {};
    budget -= variants.length - 1;
    if (budget < 0) throw new RangeError("response_schema_limit");
    if (nullable) {
      if (version === "3.0") variants[0]!.nullable = true;
      else if (variants.length === 1) variants[0]!.type = [variants[0]!.type, "null"];
      else variants.push({ type: "null" });
    }
    return variants.length === 1 ? variants[0]! : { anyOf: variants };
  }
  try {
    return infer([value], 0);
  } catch (error) {
    if (error instanceof RangeError) return null;
    throw error;
  }
}
