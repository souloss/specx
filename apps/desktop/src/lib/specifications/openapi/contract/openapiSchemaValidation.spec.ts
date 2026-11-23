import { describe, expect, it } from "vitest";
import { validateOpenApiSchema } from "./openapiSchemaValidation";

describe("OpenAPI contract validation", () => {
  it.each([
    [{ type: "string", enum: ["active", "pending"] }, "invalid", "enum"],
    [{ type: "string", minLength: 2 }, "a", "minLength"],
    [{ type: "string", pattern: "^[a-z]+$" }, "123", "pattern"],
    [{ type: "string", format: "email" }, "not-an-email", "format"],
    [{ type: "integer", minimum: 1, exclusiveMinimum: true }, 1, "exclusiveMinimum"],
    [{ type: "number", maximum: 5 }, 6, "maximum"],
    [{ type: "array", items: { type: "integer" }, uniqueItems: true }, [1, 1], "uniqueItems"],
    [{ type: "array", items: { type: "integer" }, minItems: 1 }, [], "minItems"],
    [{ type: "object", properties: { name: { type: "string" } }, additionalProperties: false }, { name: "Ada", extra: 1 }, "additionalProperties"],
    [{ type: "object", additionalProperties: { type: "integer" } }, { count: "bad" }, "type"],
    [{ type: "object", minProperties: 1 }, {}, "minProperties"],
    [{ type: "integer", minimum: 10, oneOf: [{ type: "integer" }, { type: "string" }] }, 3, "minimum"],
  ])("checks the designed constraint %j", (schema, body, keyword) => {
    const result = validateOpenApiSchema({ document: { openapi: "3.0.3" }, schema, body });
    expect(result.valid).toBe(false);
    expect(result.failure).toBeUndefined();
    expect(result.issues.some((issue) => issue.keyword === keyword)).toBe(true);
  });

  it("validates nested recursive models instead of skipping repeated references", () => {
    const node = { type: "object", required: ["value"], properties: { value: { type: "integer" }, next: { $ref: "#/components/schemas/Node" } } };
    const document = { openapi: "3.1.0", components: { schemas: { Node: node } } };
    const schema = { $ref: "#/components/schemas/Node" };
    expect(validateOpenApiSchema({ document, schema, body: { value: 1, next: { value: 2 } } }).valid).toBe(true);
    expect(validateOpenApiSchema({ document, schema, body: { value: 1, next: { value: "wrong" } } }).issues).toContainEqual({ path: "/next/value", keyword: "type", type: "integer" });
  });

  it("uses OpenAPI 3.0 nullable and 3.1 boolean, type union and sibling ref semantics", () => {
    expect(validateOpenApiSchema({ document: { openapi: "3.0.3" }, schema: { type: "string", nullable: true }, body: null }).valid).toBe(true);
    expect(validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema: false, body: null }).valid).toBe(false);
    expect(validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema: { type: ["string", "null"] }, body: null }).valid).toBe(true);
    expect(validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema: { type: "string", nullable: true }, body: null }).valid).toBe(false);
    const components = { schemas: { Name: { type: "string" } } };
    const schema = { $ref: "#/components/schemas/Name", minLength: 3 };
    expect(validateOpenApiSchema({ document: { openapi: "3.0.3", components }, schema, body: "a" }).valid).toBe(true);
    expect(validateOpenApiSchema({ document: { openapi: "3.1.0", components }, schema, body: "a" }).issues[0]?.keyword).toBe("minLength");
    expect(validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema: { type: "number", exclusiveMinimum: 1 }, body: 1 }).issues[0]?.keyword).toBe("exclusiveMinimum");
  });

  it("applies readOnly and writeOnly required fields to their declared direction", () => {
    const schema = { type: "object", required: ["id", "password", "name"], properties: { id: { type: "integer", readOnly: true }, password: { type: "string", writeOnly: true }, name: { type: "string" } } };
    const document = { openapi: "3.0.3" };
    expect(validateOpenApiSchema({ document, schema, body: { password: "value", name: "Ada" }, direction: "request" }).valid).toBe(true);
    expect(validateOpenApiSchema({ document, schema, body: { id: 1, name: "Ada" }, direction: "response" }).valid).toBe(true);
    expect(validateOpenApiSchema({ document, schema, body: { id: 1 }, direction: "response" }).issues).toContainEqual({ path: "/", keyword: "required", property: "name" });
    const modern = { openapi: "3.1.0", components: { schemas: { Id: { type: "integer" } } } };
    expect(validateOpenApiSchema({ document: modern, schema: { type: "object", required: ["id"], properties: { id: { $ref: "#/components/schemas/Id", readOnly: true } } }, body: {}, direction: "request" }).valid).toBe(true);
  });

  it("reports unavailable schemas and never exposes response or schema values", () => {
    for (const schema of [{ $ref: "https://example.test/schema" }, { $ref: "#/components/schemas/Missing" }, { type: "string", pattern: "[" }]) {
      const result = validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema, body: "PRIVATE-RESPONSE" });
      expect(result).toEqual({ valid: false, issues: [], failure: "schema" });
    }
    const result = validateOpenApiSchema({ document: { openapi: "3.1.0" }, schema: { enum: ["PRIVATE-SCHEMA"] }, body: "PRIVATE-RESPONSE" });
    expect(JSON.stringify(result)).not.toMatch(/PRIVATE/);
  });
});
