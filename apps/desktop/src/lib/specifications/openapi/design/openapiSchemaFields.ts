import { isRecord, materializeOpenApiSchema, type OpenApiObject } from "../contract/openapi";

export const schemaFieldTypes = ["string", "integer", "number", "boolean", "array", "object"] as const;
export const isOpenApi31 = (document?: OpenApiObject | null) => typeof document?.openapi === "string" && document.openapi.startsWith("3.1");
export const schemaAllowsNull = (schema: OpenApiObject) => schema.nullable === true || (Array.isArray(schema.type) && schema.type.includes("null"));

export function schemaFieldType(schema: OpenApiObject): string {
  if (typeof schema.type === "string") return schema.type;
  if (Array.isArray(schema.type)) {
    const types = schema.type.filter((type) => type !== "null");
    return types.length === 1 && typeof types[0] === "string" ? types[0] : "";
  }
  if (isRecord(schema.properties) || "additionalProperties" in schema) return "object";
  if ("items" in schema) return "array";
  return ["allOf", "oneOf", "anyOf", "$ref"].some((key) => key in schema) ? "" : "string";
}

export const schemaTypeConstraints = {
  string: ["minLength", "maxLength", "pattern", "contentEncoding", "contentMediaType"],
  numeric: ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf"],
  array: ["items", "minItems", "maxItems", "uniqueItems", "prefixItems", "contains", "minContains", "maxContains"],
  object: ["properties", "required", "additionalProperties", "minProperties", "maxProperties", "patternProperties", "propertyNames", "dependentRequired", "dependentSchemas"],
};

export function changeSchemaFieldType(schema: OpenApiObject, type: string, document?: OpenApiObject | null): OpenApiObject {
  const next = { ...materializeOpenApiSchema(document ?? null, schema) };
  delete next.$ref;
  const branch = schemaModelReference(schema);
  if (branch && typeof schema.$ref !== "string") delete next.allOf;
  const nullable = schemaAllowsNull(schema);
  next.type = nullable && isOpenApi31(document) ? [type, "null"] : type;
  if (isOpenApi31(document)) delete next.nullable;
  const group = type === "number" || type === "integer" ? "numeric" : type;
  for (const [kind, keys] of Object.entries(schemaTypeConstraints)) {
    if (kind !== group) for (const key of keys) delete next[key];
  }
  if (type === "object") next.properties ??= {};
  if (type === "array") next.items ??= { type: "string" };
  if (Array.isArray(next.enum)) {
    next.enum = next.enum.filter((value) => schemaValueMatches(value, type, nullable));
    if (!(next.enum as unknown[]).length) delete next.enum;
    if (Array.isArray(next["x-apifox-enum"])) {
      next["x-apifox-enum"] = next["x-apifox-enum"].filter((row) => isRecord(row) && (next.enum as unknown[] | undefined)?.some((value) => sameSchemaValue(value, row.value)));
      if (!(next["x-apifox-enum"] as unknown[]).length) delete next["x-apifox-enum"];
    }
  }
  for (const key of ["default", "const"]) if (key in next && !schemaValueMatches(next[key], type, nullable)) delete next[key];
  return next;
}

export function schemaModelReference(schema: OpenApiObject): string {
  if (typeof schema.$ref === "string") return schema.$ref;
  const branches = schema.allOf;
  return Array.isArray(branches) && branches.length === 1 && isRecord(branches[0]) && Object.keys(branches[0]).length === 1 && typeof branches[0].$ref === "string" ? branches[0].$ref : "";
}

export function schemaModelConstraints(schema: OpenApiObject): OpenApiObject {
  const constraints = { ...schema };
  if (typeof schema.$ref === "string") delete constraints.$ref;
  else if (schemaModelReference(schema)) delete constraints.allOf;
  return constraints;
}

export function withSchemaModel(schema: OpenApiObject, reference: string, document?: OpenApiObject | null): OpenApiObject {
  const constraints = schemaModelConstraints(schema);
  if (isOpenApi31(document)) return { $ref: reference, ...constraints };
  return Object.keys(constraints).length ? { allOf: [{ $ref: reference }], ...constraints } : { $ref: reference };
}

export function schemaValueMatches(value: unknown, type: string, nullable: boolean): boolean {
  if (value === null) return nullable || type === "null" || !type;
  switch (type) {
    case "array":
      return Array.isArray(value);
    case "object":
      return isRecord(value);
    case "integer":
      return Number.isInteger(value);
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    default:
      return !type || typeof value === type;
  }
}

export function schemaValueText(value: unknown): string {
  return value === undefined ? "" : typeof value === "string" ? value : JSON.stringify(value);
}

export function parseSchemaFieldValue(text: string, type: string, nullable: boolean): { value: unknown } | undefined {
  try {
    const value: unknown = type === "string" && !(nullable && text === "null") && !text.startsWith('"') ? text : JSON.parse(text);
    return schemaValueMatches(value, type, nullable) ? { value } : undefined;
  } catch {
    return undefined;
  }
}

export const sameSchemaValue = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);

export function constraintValueText(value: unknown, nullable: boolean): string {
  return typeof value === "string" && (value === "" || value.startsWith('"') || (nullable && value === "null")) ? JSON.stringify(value) : schemaValueText(value);
}
