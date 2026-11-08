import { describe, expect, it } from "vitest";
import { formatOpenApiContent, getOpenApiOperations, parseOpenApiContent } from "./openapi";

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

  it("rejects malformed content for format actions", () => {
    expect(parseOpenApiContent("openapi: [")).toBeNull();
    expect(() => formatOpenApiContent("openapi: [", "json")).toThrow("invalid_document");
  });
});
