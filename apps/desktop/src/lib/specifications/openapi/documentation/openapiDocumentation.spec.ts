import { describe, expect, it } from "vitest";
import { getOpenApiOperations, type OpenApiObject } from "../contract/openapi";
import { getOperationParameters } from "../contract/openapiParameters";
import { operationExampleRequest, operationMarkdown } from "./openapiDocumentation";
import { buildCodeSnippet } from "../snippets/openapiCodegen";

describe("operation documentation exports", () => {
  const document: OpenApiObject = {
    openapi: "3.1.0",
    servers: [{ url: "https://{region}.example.test/v2", variables: { region: { default: "eu" } } }],
    security: [{ Token: [] }],
    components: {
      securitySchemes: { Token: { type: "http", scheme: "bearer" } },
      parameters: { Limit: { in: "query", name: "limit", example: 9, schema: { type: "integer" } } },
      schemas: {
        Pet: { type: "object", properties: { id: { type: "integer" }, parent: { $ref: "#/components/schemas/Pet" } } },
        Unrelated: { type: "string" },
      },
    },
    paths: {
      "/pets/{id}": {
        parameters: [{ $ref: "#/components/parameters/Limit" }, { in: "path", name: "id", example: "a/b", schema: { type: "string" } }],
        get: {
          summary: "Read pet",
          parameters: [
            { in: "query", name: "limit", example: 0, schema: { type: "integer" } },
            { in: "query", name: "tags", example: ["cat", "dog"], schema: { type: "array", items: { type: "string" } } },
          ],
          responses: { "200": { description: "OK", content: { "application/json": { schema: { $ref: "#/components/schemas/Pet" } } } } },
        },
      },
    },
  };
  const entry = getOpenApiOperations(JSON.stringify(document))[0]!;

  it("inherits referenced parameters, keeps operation overrides and serializes the same contract as the workbench", () => {
    expect(getOperationParameters(document, entry).filter((parameter) => parameter.name === "limit")).toHaveLength(1);
    const request = operationExampleRequest(document, entry);
    expect(request.url).toBe("https://eu.example.test/v2/pets/a%2Fb?limit=0&tags=cat&tags=dog");
    expect(request.headers).toEqual([{ name: "Authorization", value: "Bearer YOUR_ACCESS_TOKEN", enabled: true }]);
  });

  it("uses an explicit illustrative origin for sanitized publications and honors anonymous operations", () => {
    const publicDocument = { ...document, servers: undefined };
    const request = operationExampleRequest(publicDocument, { ...entry, operation: { ...entry.operation, security: [] } });
    expect(request.url).toMatch(/^https:\/\/api\.example\.com\//);
    expect(request.headers).toEqual([]);
  });

  it.each(["application/x-www-form-urlencoded", "multipart/form-data"])("generates valid %s fields with repeated values", (mediaType) => {
    const request = operationExampleRequest(null, { ...entry, method: "post", operation: { requestBody: { content: { [mediaType]: { example: { tag: ["a b", "c"], enabled: false } } } } } });
    expect([...new URLSearchParams(request.body)]).toEqual([
      ["tag", "a b"],
      ["tag", "c"],
      ["enabled", "false"],
    ]);
    const snippet = buildCodeSnippet(request, "curl");
    expect(snippet).toContain(mediaType === "multipart/form-data" ? "--form-string 'tag=a b'" : "tag=a+b&tag=c&enabled=false");
  });

  it("exports only this operation and its recursive reference closure", () => {
    const markdown = operationMarkdown(document, entry);
    expect(markdown).toContain("# Read pet");
    expect(markdown).toContain('"id"');
    expect(markdown).toContain("#/components/schemas/Pet");
    expect(markdown).not.toContain("Unrelated");
    expect(markdown).toContain("## Authentication");
    expect(markdown).toContain('"scheme": "bearer"');
    expect(markdown.length).toBeLessThan(4000);
  });

  it("generates the selected media and named example, preserving explicit null examples", () => {
    const operation = { requestBody: { content: { "application/json": { examples: { empty: { value: null }, pet: { value: { name: "Cat" } } } }, "text/plain": { example: "A plain pet" } } } };
    const selectedEntry = { ...entry, operation };
    expect(operationExampleRequest(null, selectedEntry, { mediaType: "text/plain", exampleName: "default" })).toMatchObject({ mediaType: "text/plain", body: "A plain pet" });
    expect(operationExampleRequest(null, selectedEntry, { mediaType: "application/json", exampleName: "pet" }).body).toContain('"Cat"');
    expect(operationExampleRequest(null, selectedEntry, { mediaType: "application/json", exampleName: "empty" }).body.trim()).toBe("null");
  });
});
