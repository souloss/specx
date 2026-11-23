import Ajv from "ajv";
import Ajv2020 from "ajv/dist/2020";
import addFormats from "ajv-formats";
import { isRecord, resolveOpenApiRef, type OpenApiObject } from "./openapi";

export type OpenApiSchemaValidationInput = {
  document: OpenApiObject | null;
  schema: unknown;
  body: unknown;
  direction?: "request" | "response";
};
export type OpenApiSchemaIssue = { path: string; keyword: string; property?: string; type?: string; limit?: number; format?: string };
export type OpenApiSchemaValidationResult = { valid: boolean; issues: OpenApiSchemaIssue[]; failure?: "schema" | "timeout" | "worker" };

/** Validate the contract itself, including compositions and local references.
 * Production callers run this in a disposable Worker: schema compilation and
 * regular expressions must not block the editor's UI thread.
 */
export function validateOpenApiSchema(input: OpenApiSchemaValidationInput): OpenApiSchemaValidationResult {
  const { document, schema, body, direction = "response" } = input;
  const modern = typeof document?.openapi === "string" && document.openapi.startsWith("3.1");
  const prepareSchema = (value: unknown): unknown => {
    if (!isRecord(value)) return value;
    // In 3.0 a Reference Object's siblings are ignored. 3.1 Schema Objects
    // follow JSON Schema and apply sibling constraints alongside the reference.
    if (!modern && typeof value.$ref === "string") return { $ref: value.$ref };
    const result: OpenApiObject = { ...value };
    if (modern) delete result.nullable;
    if (!modern) {
      for (const [bound, exclusive] of [
        ["minimum", "exclusiveMinimum"],
        ["maximum", "exclusiveMaximum"],
      ] as const) {
        if (typeof result[exclusive] !== "boolean") continue;
        if (result[exclusive] && typeof result[bound] === "number") {
          result[exclusive] = result[bound];
          delete result[bound];
        } else delete result[exclusive];
      }
    }
    if (Array.isArray(value.required) && isRecord(value.properties)) {
      const properties = value.properties;
      result.required = value.required.filter((key) => {
        if (typeof key !== "string") return true;
        const rawProperty = properties[key];
        const resolved = resolveOpenApiRef(document, rawProperty);
        const property = modern && isRecord(rawProperty) ? { ...resolved, ...rawProperty } : resolved;
        return !(direction === "request" ? property?.readOnly === true : property?.writeOnly === true);
      });
    }
    for (const key of ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas"]) {
      if (isRecord(value[key])) result[key] = Object.fromEntries(Object.entries(value[key]).map(([name, child]) => [name, prepareSchema(child)]));
    }
    for (const key of ["allOf", "anyOf", "oneOf", "prefixItems"]) {
      if (Array.isArray(value[key])) result[key] = value[key].map(prepareSchema);
    }
    for (const key of ["items", "additionalProperties", "unevaluatedProperties", "additionalItems", "contains", "propertyNames", "not", "if", "then", "else"]) {
      if (key in value) result[key] = Array.isArray(value[key]) ? value[key].map(prepareSchema) : prepareSchema(value[key]);
    }
    return result;
  };
  const prepareDocument = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(prepareDocument);
    if (!isRecord(value)) return value;
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [key, key === "schema" ? prepareSchema(child) : ["schemas", "definitions", "$defs"].includes(key) && isRecord(child) ? Object.fromEntries(Object.entries(child).map(([name, item]) => [name, prepareSchema(item)])) : prepareDocument(child)]),
    );
  };
  try {
    const root = prepareDocument(document ?? {}) as OpenApiObject;
    // Keep document-relative JSON Pointers intact without fetching remote refs.
    delete root.$id;
    delete root.$schema;
    let slot = "x-specx-validation";
    while (Object.hasOwn(root, slot)) slot += "_";
    root[slot] = prepareSchema(schema);
    root.$ref = `#/${slot}`;
    const options = { strict: false, validateSchema: true, inlineRefs: false, verbose: false, messages: false, logger: false as const };
    const validator = modern ? new Ajv2020(options) : new Ajv(options);
    addFormats(validator);
    const validate = validator.compile(root);
    const valid = validate(body) === true;
    const issues = (validate.errors ?? []).slice(0, 50).map((error): OpenApiSchemaIssue => {
      const params = error.params as Record<string, unknown>;
      return {
        path: error.instancePath || "/",
        keyword: error.keyword,
        ...(typeof params.missingProperty === "string" ? { property: params.missingProperty } : typeof params.additionalProperty === "string" ? { property: params.additionalProperty } : {}),
        ...(typeof params.type === "string" ? { type: params.type } : {}),
        ...(typeof params.limit === "number" ? { limit: params.limit } : {}),
        ...(typeof params.format === "string" ? { format: params.format } : {}),
      };
    });
    return { valid, issues };
  } catch {
    // Never expose compiled source, schema contents or response values in errors.
    return { valid: false, issues: [], failure: "schema" };
  }
}
