import { describe, expect, it } from "vitest";
import { expandOpenApiServerUrl, formatOpenApiContent, formatOpenApiExample, generateOpenApiExample, getOpenApiMediaExamples, getOpenApiOperations, getOpenApiServers, materializeOpenApiSchema, mergeOpenApiSchema, parseOpenApiContent, resolveOpenApiRef } from "./openapi";
import { validateOpenApiSchema } from "./openapiSchemaValidation";

const yamlDocument = "openapi: 3.1.0\ninfo:\n  title: Accounts\n  version: '1'\npaths:\n  /accounts:\n    get:\n      tags: [Accounts]\n";

describe("OpenAPI browser helpers", () => {
  it("parses YAML and derives stable Tag operation identities", () => {
    expect(getOpenApiOperations(yamlDocument).map((operation) => [operation.identityKey, operation.tags])).toEqual([["GET:/accounts", ["Accounts"]]]);
  });

  it("formats between JSON and YAML without changing the document", () => {
    const json = formatOpenApiContent(yamlDocument, "json");
    expect(parseOpenApiContent(json)?.openapi).toBe("3.1.0");
    expect(formatOpenApiContent(json, "yaml")).toContain("title: Accounts");
  });

  it("expands OpenAPI server variables with environment values before defaults", () => {
    const document = parseOpenApiContent(`openapi: 3.0.3
servers:
  - url: https://{region}.example.test/{version}
    variables:
      region: { default: us }
      version: { default: v1 }
`)!;
    expect(expandOpenApiServerUrl(document, "https://{region}.example.test/{version}", { region: "eu" })).toBe("https://eu.example.test/v1");
  });

  it("rejects malformed content for format actions", () => {
    expect(parseOpenApiContent("openapi: [")).toBeNull();
    expect(() => formatOpenApiContent("openapi: [", "json")).toThrow("invalid_document");
  });

  it("resolves server precedence and variable defaults for each operation", () => {
    const root = { url: "https://root.example.test" };
    const path = { url: "https://path.example.test/{version}", variables: { version: { default: "v2" } } };
    const operation = { url: "https://operation.example.test/{version}", variables: { version: { default: "v3" } } };
    const document = { servers: [root], paths: { "/health": { servers: [path], get: { servers: [operation] }, post: {}, delete: { servers: [] } }, "/root": { get: {} } } };
    const entries = getOpenApiOperations(JSON.stringify(document));
    const get = entries.find((entry) => entry.identityKey === "GET:/health")!;
    const post = entries.find((entry) => entry.identityKey === "POST:/health")!;
    expect(getOpenApiServers(document, get)).toEqual([operation]);
    expect(getOpenApiServers(document, post)).toEqual([path]);
    expect(
      getOpenApiServers(
        document,
        entries.find((entry) => entry.identityKey === "GET:/root"),
      ),
    ).toEqual([root]);
    expect(
      getOpenApiServers(
        document,
        entries.find((entry) => entry.identityKey === "DELETE:/health"),
      ),
    ).toEqual([]);
    expect(expandOpenApiServerUrl(document, operation.url, {}, get)).toBe("https://operation.example.test/v3");
    expect(expandOpenApiServerUrl(document, path.url, { version: "preview" }, post)).toBe("https://path.example.test/preview");
  });

  it("generates a bounded example and validates a response against a local ref", () => {
    const document = parseOpenApiContent(`openapi: 3.0.3
components:
  schemas:
    User:
      type: object
      required: [id]
      properties:
        id: { type: integer }
        name: { type: string }
`)!;
    const schema = { $ref: "#/components/schemas/User" };
    expect(generateOpenApiExample(document, schema)).toEqual({ id: 0, name: "string" });
    expect(validateOpenApiSchema({ document, schema, body: { id: 1 } }).valid).toBe(true);
    expect(validateOpenApiSchema({ document, schema, body: { name: "Ada" } }).issues).toContainEqual({ path: "/", keyword: "required", property: "id" });
  });

  it("resolves named media examples and serializes them for their declared media type", () => {
    const document = parseOpenApiContent(`openapi: 3.0.3
components:
  examples:
    Unauthorized:
      summary: Missing credentials
      description: The caller did not send a bearer token.
      value: { code: "unauthorized", message: "Sign in first" }
paths: {}
`)!;
    const examples = getOpenApiMediaExamples(document, {
      example: { code: "fallback" },
      examples: {
        unauthorized: { $ref: "#/components/examples/Unauthorized" },
        forbidden: { summary: "Not allowed", value: { code: "forbidden" } },
        remote: { externalValue: "https://example.test/secret.json" },
      },
    });
    expect(examples).toEqual([
      { name: "default", label: "Example", description: "", value: { code: "fallback" } },
      { name: "unauthorized", label: "Missing credentials", description: "The caller did not send a bearer token.", value: { code: "unauthorized", message: "Sign in first" } },
      { name: "forbidden", label: "Not allowed", description: "", value: { code: "forbidden" } },
    ]);
    expect(formatOpenApiExample({ code: "unauthorized" }, "application/json")).toBe('{\n  "code": "unauthorized"\n}\n');
    expect(formatOpenApiExample({ code: "unauthorized" }, "application/yaml")).toContain("code: unauthorized");
    expect(formatOpenApiExample("plain body", "text/plain")).toBe("plain body");
  });

  it("follows chained local refs used by generated API error envelopes", () => {
    const document = parseOpenApiContent(`openapi: 3.0.3
components:
  schemas:
    ErrorEnvelope:
      $ref: '#/components/schemas/ErrorResponse'
    ErrorResponse:
      allOf:
        - $ref: '#/components/schemas/ErrorFields'
    ErrorFields:
      type: object
      required: [code]
      properties:
        code: { type: string }
`)!;
    const schema = { $ref: "#/components/schemas/ErrorEnvelope" };
    expect(generateOpenApiExample(document, schema)).toEqual({ code: "string" });
    expect(validateOpenApiSchema({ document, schema, body: { code: "invalid_request" } }).valid).toBe(true);
  });

  it("flattens composed response schemas so inherited fields remain visible", () => {
    const document = parseOpenApiContent(`openapi: 3.0.3
components:
  schemas:
    ErrorResponse:
      allOf:
        - $ref: '#/components/schemas/ErrorFields'
        - type: object
          properties:
            traceId: { type: string }
    ErrorFields:
      type: object
      required: [code]
      properties:
        code: { type: string }
    GlobalCredential:
      type: object
      properties:
        name: { type: string }
`)!;
    expect(mergeOpenApiSchema(document, { $ref: "#/components/schemas/ErrorResponse" })).toMatchObject({
      type: "object",
      properties: { code: { type: "string" }, traceId: { type: "string" } },
      required: ["code"],
    });
    expect(mergeOpenApiSchema(document, { $ref: "#/components/schemas/GlobalCredential" })?.properties).toEqual({ name: { type: "string" } });
  });

  it("resolves Swagger 2 definitions and JSON Pointer escaped schema names", () => {
    const document = parseOpenApiContent(`swagger: '2.0'
definitions:
  Error/Response:
    type: object
    properties:
      code: { type: string }
  GlobalCredential:
    allOf:
      - $ref: '#/definitions/Error~1Response'
      - type: object
        properties:
          secret: { type: string }
`)!;
    expect(mergeOpenApiSchema(document, { $ref: "#/definitions/Error~1Response" })?.properties).toEqual({ code: { type: "string" } });
    expect(mergeOpenApiSchema(document, { $ref: "#/definitions/GlobalCredential" })?.properties).toEqual({ code: { type: "string" }, secret: { type: "string" } });
  });

  it("resolves local array pointers and excludes inherited object properties", () => {
    const document = { components: { schemas: { Name: { allOf: [{ type: "string" }] } } } };
    expect(resolveOpenApiRef(document, { $ref: "#/components/schemas/Name/allOf/0" })).toEqual({ type: "string" });
    expect(resolveOpenApiRef(document, { $ref: "#/__proto__" })).toEqual({ $ref: "#/__proto__" });
  });

  it("retains each OpenAPI 3.1 ref sibling constraint when materializing an edited schema", () => {
    const components = { schemas: { Name: { type: "string", minLength: 3 }, Alias: { $ref: "#/components/schemas/Name", maxLength: 6 } } };
    const source = { $ref: "#/components/schemas/Alias", minLength: 1 };
    const document = { openapi: "3.1.0", components };
    const materialized = materializeOpenApiSchema(document, source);
    expect(materialized).toEqual({ allOf: [{ type: "string", minLength: 3 }, { minLength: 1 }, { maxLength: 6 }] });
    expect(validateOpenApiSchema({ document, schema: materialized, body: "ab" }).valid).toBe(false);
    expect(validateOpenApiSchema({ document, schema: materialized, body: "valid" }).valid).toBe(true);
    expect(validateOpenApiSchema({ document, schema: materialized, body: "too long" }).valid).toBe(false);
    expect(materializeOpenApiSchema({ openapi: "3.0.3", components }, source)).toEqual({ type: "string", minLength: 3 });
    expect(components.schemas.Alias).toEqual({ $ref: "#/components/schemas/Name", maxLength: 6 });
  });
});
