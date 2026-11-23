import { describe, expect, it } from "vitest";
import { getOpenApiOperations } from "../contract/openapi";
import { getOpenApiModels } from "../contract/openapiModels";
import { documentMarkdown, documentationHtml, documentationIndex, modelMarkdown, openApiYaml, operationOpenApiDocument } from "./openapiDocumentExport";
import { parse } from "yaml";

const document = {
  openapi: "3.1.0",
  info: { title: "Pets", version: "1" },
  servers: [{ url: "https://api.example.test" }],
  security: [{ bearer: [] }],
  paths: {
    "/pets/{id}": {
      servers: [{ url: "https://pets.example.test" }],
      parameters: [
        { in: "path", name: "id", required: true, schema: { type: "string" } },
        { in: "query", name: "q", schema: { type: "string" } },
      ],
      get: { parameters: [{ in: "query", name: "q", schema: { type: "integer" } }], responses: { "200": { description: "Pet", content: { "application/json": { schema: { $ref: "#/components/schemas/Pet" }, example: { $ref: "literal" } } } } } },
      delete: { security: [], responses: { "204": { description: "Deleted" } } },
    },
    "/unrelated": { get: { responses: {} } },
  },
  components: {
    securitySchemes: { bearer: { type: "http", scheme: "bearer" }, unused: { type: "http", scheme: "basic" } },
    schemas: { Pet: { type: "object", properties: { friend: { $ref: "#/components/schemas/Pet" }, state: { $ref: "#/components/schemas/State" } } }, State: { type: "string", enum: ["ok"] }, Unused: { type: "boolean" } },
  },
};

describe("portable OpenAPI document export", () => {
  it("isolates an operation and its cyclic reference closure, effective parameters, servers and auth without modifying the source", () => {
    const source = JSON.stringify(document);
    const entry = getOpenApiOperations(source).find((entry) => entry.method === "get" && entry.path === "/pets/{id}")!;
    const result = operationOpenApiDocument(document, entry);
    expect(result).toMatchObject({ openapi: "3.1.0", servers: document.servers, security: document.security, paths: { "/pets/{id}": { servers: document.paths["/pets/{id}"].servers, parameters: [{ name: "id" }, { name: "q", schema: { type: "integer" } }] } } });
    expect(Object.keys(result.paths as object)).toEqual(["/pets/{id}"]);
    expect(result.paths).not.toHaveProperty("/pets/{id}.delete");
    expect(result.components).toEqual({ securitySchemes: { bearer: document.components.securitySchemes.bearer }, schemas: { Pet: document.components.schemas.Pet, State: document.components.schemas.State } });
    expect(JSON.stringify(document)).toBe(source);
    expect(parse(openApiYaml(result))).toEqual(result);
  });

  it("preserves explicit anonymous security and excludes unused inherited schemes", () => {
    const entry = getOpenApiOperations(JSON.stringify(document)).find((entry) => entry.method === "delete")!;
    const result = operationOpenApiDocument(document, entry);
    expect(result.security).toEqual([]);
    expect(result.components).toBeUndefined();
  });

  it("includes raw referenced fields in copied model Markdown and creates deep links in the downloaded index", () => {
    const model = getOpenApiModels(document).find((model) => model.name === "Pet")!;
    const markdown = modelMarkdown(model, document);
    expect(markdown).toContain('"enum": [');
    expect(markdown).not.toContain("Unused");
    const all = documentMarkdown(document, "Pets");
    expect(all).toContain("DELETE /pets/{id}");
    expect(all).toContain("# State");
    const index = documentationIndex(document, "Pets", "https://docs.example.test/#old");
    expect(index).toContain("https://docs.example.test/#GET%3A%2Fpets%2F%7Bid%7D");
    expect(index).toContain("#model%3A%23%2Fcomponents%2Fschemas%2FPet");
    expect(index).not.toContain("#old");
  });

  it("sanitizes exported HTML and escapes its title", () => {
    const html = documentationHtml('# API\n\n<script>alert(1)</script>\n\n<img src=x onerror="alert(2)">\n\n[bad](javascript:alert(3))', "</title><script>alert(4)</script>");
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("onerror");
    expect(html).not.toContain("javascript:");
    expect(html).toContain("&lt;/title&gt;");
    expect(html).toContain("<h1>API</h1>");
  });
});
