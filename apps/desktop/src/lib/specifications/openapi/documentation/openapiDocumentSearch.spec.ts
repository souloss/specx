import { describe, expect, it } from "vitest";
import { getOpenApiOperations } from "../contract/openapi";
import { getOpenApiModels } from "../contract/openapiModels";
import { buildOpenApiSearchIndex, matchesOpenApiSearch } from "./openapiDocumentSearch";

describe("document field search", () => {
  it("finds inherited parameters and referenced fields, including keyword field names, without indexing payloads", () => {
    const document = {
      openapi: "3.1.0",
      paths: { "/nodes": { parameters: [{ name: "tenantSlug", in: "query", schema: { type: "string" } }], get: { responses: { "200": { content: { "application/json": { schema: { $ref: "#/components/schemas/Node" }, example: { hiddenPayload: "privateSample" } } } } } } } },
      components: { schemas: { Node: { type: "object", properties: { example: { description: "literalField", type: "string" }, value: { type: "number" }, "x-id": { type: "string" }, child: { $ref: "#/components/schemas/Node" } } } } },
    };
    const operations = getOpenApiOperations(JSON.stringify(document));
    const index = buildOpenApiSearchIndex(document, operations, getOpenApiModels(document));
    const text = index.operations.get(operations[0]!.identityKey)!;
    expect(matchesOpenApiSearch(text, "GET tenantSlug literalField value x-id")).toBe(true);
    expect(matchesOpenApiSearch(text, "privateSample")).toBe(false);
    expect(matchesOpenApiSearch(index.models.get("#/components/schemas/Node")!, "example child")).toBe(true);
    expect(matchesOpenApiSearch(text, "tenantSlug missing")).toBe(false);
  });
});
