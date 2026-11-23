import { describe, expect, it } from "vitest";
import { copyPublicEnvironment, readPublicEnvironments, validPublicEnvironments, withPublicEnvironments } from "./openapiPublicEnvironments";
import { operationOpenApiDocument, openApiYaml } from "../documentation/openapiDocumentExport";
import { getOpenApiOperations, parseOpenApiContent } from "../contract/openapi";

const environments = [
  {
    name: "Demo",
    baseUrl: "https://{{host}}",
    variables: [
      { key: "host", value: "jsonplaceholder.typicode.com" },
      { key: "userId", value: "1" },
    ],
  },
];
describe("portable public environments", () => {
  it("round trips public domains and ordinary variables through whole and single-operation JSON/YAML exports", () => {
    const source = JSON.stringify({ openapi: "3.1.0", info: { title: "Posts", version: "1" }, paths: { "/posts": { get: { responses: { "200": { description: "OK" } } } } } });
    const content = withPublicEnvironments(source, environments);
    const document = parseOpenApiContent(content)!;
    expect(document.servers).toEqual([{ url: "https://jsonplaceholder.typicode.com", description: "Demo" }]);
    expect(readPublicEnvironments(document)).toEqual(environments);
    const operation = operationOpenApiDocument(document, getOpenApiOperations(content)[0]!);
    expect(readPublicEnvironments(parseOpenApiContent(openApiYaml(operation)))).toEqual(environments);
    expect(source).not.toContain("environments");
  });
  it("copies only enabled non-sensitive values and never secret metadata", () => {
    expect(
      copyPublicEnvironment({
        id: "private-id",
        serviceId: "service",
        name: "Demo",
        slug: "demo",
        baseUrl: "https://example.test",
        createdAt: "",
        updatedAt: "",
        variables: [
          { key: "ordinary", value: "yes", sensitive: false, enabled: true, hasValue: true },
          { key: "secret", value: "never", sensitive: true, enabled: true, hasValue: true },
          { key: "disabled", value: "no", sensitive: false, enabled: false, hasValue: true },
          { key: "unset", value: null, sensitive: false, enabled: true, hasValue: false },
        ],
      }),
    ).toEqual({ name: "Demo", baseUrl: "https://example.test", variables: [{ key: "ordinary", value: "yes" }] });
  });
  it("rejects invalid, unresolved and credentialed domains, duplicate values, and malformed extensions", () => {
    for (const baseUrl of ["/api", "javascript:alert(1)", "https://user:pass@example.test", "https://example.test?token=x", "https://example.test/#x", "https://{{missing}}", "https://example.test/\\evil"]) expect(validPublicEnvironments([{ ...environments[0]!, baseUrl }])).toBe(false);
    expect(validPublicEnvironments([environments[0]!, { ...environments[0]!, name: "demo" }])).toBe(false);
    expect(validPublicEnvironments([{ ...environments[0]!, variables: [...environments[0]!.variables, { key: "host", value: "other.test" }] }])).toBe(false);
    expect(readPublicEnvironments({ "x-specx-public-environments": [{ name: "broken" }] })).toEqual([]);
  });
});
