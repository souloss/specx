import { describe, expect, it } from "vitest";
import { openApiPublicationUrl, publicOpenApiPublicationId } from "./openapiPublication";

const id = "a".repeat(32);

describe("OpenAPI publication URLs", () => {
  it("uses a randomized localhost subdomain for local IP deployments", () => {
    expect(openApiPublicationUrl(id, { hostname: "127.0.0.1", port: "4338", protocol: "http:", pathname: "/" })).toBe(`http://${id}.localhost:4338/`);
  });

  it("uses a randomized subdomain under the application domain", () => {
    expect(openApiPublicationUrl(id, { hostname: "docs.example.test", port: "", protocol: "https:", pathname: "/" })).toBe(`https://${id}.docs.example.test/`);
  });

  it("recognizes both host-based and legacy path-based publication URLs", () => {
    expect(publicOpenApiPublicationId({ hostname: `${id}.docs.example.test`, port: "", protocol: "https:", pathname: "/" })).toBe(id);
    expect(publicOpenApiPublicationId({ hostname: "docs.example.test", port: "", protocol: "https:", pathname: `/docs/openapi/${id}` })).toBe(id);
  });

  it("rejects malformed publication ids", () => {
    expect(() => openApiPublicationUrl("../bad", { hostname: "localhost", port: "", protocol: "http:", pathname: "/" })).toThrow("Invalid OpenAPI publication id");
  });
});
