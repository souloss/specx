import { describe, expect, it } from "vitest";
import { inferResponseSchema } from "./openapiResponseInference";
import { validateOpenApiSchema } from "../contract/openapiSchemaValidation";

describe("response schema inference", () => {
  it.each(["3.0", "3.1"] as const)("accepts mixed observations without turning optional fields into rules in OpenAPI %s", (version) => {
    const samples = [{ id: 1, details: [] }, null, { id: 1.5, note: null, details: [true, "description"] }, "pending"];
    const document = { openapi: `${version}.0` };
    const schema = inferResponseSchema(samples, version);
    const validate = (body: unknown) => validateOpenApiSchema({ document, schema, body }).valid;
    expect(validate(samples)).toBe(true);
    expect(validate([{ id: 2.5, details: [], extra: true }, {}, null, "unseen string"])).toBe(true);
    expect(validate([{ id: "bad" }])).toBe(false);
    expect(validate([5])).toBe(false);
  });

  it("leaves empty arrays and unknown 3.0 null-only fields unconstrained", () => {
    expect(inferResponseSchema({ list: [], value: null }, "3.0")).toEqual({ type: "object", properties: { list: { type: "array", items: {} }, value: {} } });
    expect(inferResponseSchema(null, "3.1")).toEqual({ type: "null" });
    expect(inferResponseSchema("plain text", "3.1")).toEqual({ type: "string" });
  });

  it("keeps literal property names and refuses a schema too large for the design editor", () => {
    const schema = inferResponseSchema(JSON.parse('{"__proto__":true,"fields.with.dots":0}'), "3.1");
    expect(JSON.stringify(schema)).toContain('"__proto__":{"type":"boolean"}');
    expect(inferResponseSchema(Object.fromEntries(Array.from({ length: 200 }, (_, index) => [`key${index}`, index])), "3.1")).toBeNull();
  });
});
