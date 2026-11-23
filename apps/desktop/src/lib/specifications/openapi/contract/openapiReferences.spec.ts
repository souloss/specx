import { describe, expect, it } from "vitest";
import { replaceOpenApiSchemaReference } from "./openapiReferences";

describe("OpenAPI Schema reference replacement", () => {
  it("retains Schema Object sibling constraints and annotations in OpenAPI 3.1", () => {
    const source = { $ref: "#/components/schemas/Old", description: "Local rule", minLength: 4, "x-owner": "contract" };
    const result = replaceOpenApiSchemaReference({ openapi: "3.1.0" }, source, "#/components/schemas/Next");
    expect(result).toEqual({ $ref: "#/components/schemas/Next", description: "Local rule", minLength: 4, "x-owner": "contract" });
    expect(source.$ref).toBe("#/components/schemas/Old");
  });

  it("drops siblings that OpenAPI 3.0 does not assign Schema Object semantics", () => {
    expect(replaceOpenApiSchemaReference({ openapi: "3.0.3" }, { $ref: "#/components/schemas/Old", description: "Ignored", minLength: 4 }, "#/components/schemas/Next")).toEqual({ $ref: "#/components/schemas/Next" });
  });

  it("selecting a model from an inline schema replaces its local structure", () => {
    expect(replaceOpenApiSchemaReference({ openapi: "3.1.0" }, { type: "object", properties: { name: { type: "string" } } }, "#/components/schemas/Person")).toEqual({ $ref: "#/components/schemas/Person" });
  });
});
