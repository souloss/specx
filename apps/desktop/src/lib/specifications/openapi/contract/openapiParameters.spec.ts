import { describe, expect, it } from "vitest";
import { readOpenApiPathParameter, readOpenApiQueryParameter, serializeOpenApiParameter } from "./openapiParameters";

function serialize(parameter: Record<string, unknown>, value: string) {
  const result = serializeOpenApiParameter(parameter, value);
  if (!result.ok) throw new Error(result.error);
  return result.value;
}

describe("OpenAPI parameter serialization", () => {
  it("uses form/explode defaults for arrays and objects in query parameters", () => {
    expect(serialize({ in: "query", name: "color", schema: { type: "array" } }, "red,blue").queryEntries).toEqual([
      ["color", "red"],
      ["color", "blue"],
    ]);
    expect(serialize({ in: "query", name: "filter", schema: { type: "object" } }, '{"role":"admin","active":true}').queryEntries).toEqual([
      ["role", "admin"],
      ["active", "true"],
    ]);
  });

  it("serializes non-exploded, space-delimited, pipe-delimited and deep objects", () => {
    expect(serialize({ in: "query", name: "color", style: "form", explode: false, schema: { type: "array" } }, "red,blue").queryEntries).toEqual([["color", "red,blue"]]);
    expect(serialize({ in: "query", name: "color", style: "spaceDelimited", explode: false, schema: { type: "array" } }, "red blue").queryEntries).toEqual([["color", "red%20blue"]]);
    expect(serialize({ in: "query", name: "color", style: "pipeDelimited", explode: false, schema: { type: "array" } }, "red|blue").queryEntries).toEqual([["color", "red|blue"]]);
    expect(serialize({ in: "query", name: "filter", style: "deepObject", schema: { type: "object" } }, '{"role":"admin"}').queryEntries).toEqual([["filter%5Brole%5D", "admin"]]);
  });

  it("serializes simple, label and matrix path parameters", () => {
    expect(serialize({ in: "path", name: "id", schema: { type: "array" } }, "a,b").pathValue).toBe("a,b");
    expect(serialize({ in: "path", name: "id", style: "label", explode: true, schema: { type: "array" } }, "a,b").pathValue).toBe(".a.b");
    expect(serialize({ in: "path", name: "id", style: "matrix", explode: true, schema: { type: "array" } }, "a,b").pathValue).toBe(";id=a;id=b");
    expect(serialize({ in: "path", name: "meta", style: "label", explode: true, schema: { type: "object" } }, '{"role":"admin","active":true}').pathValue).toBe(".role=admin.active=true");
    expect(serialize({ in: "path", name: "id", style: "matrix", schema: { type: "string" } }, "a/b").pathValue).toBe(";id=a%2Fb");
  });

  it("serializes header objects and cookie arrays according to their locations", () => {
    expect(serialize({ in: "header", name: "X-Meta", explode: true, schema: { type: "object" } }, '{"role":"admin","active":true}').headerValue).toBe("role=admin,active=true");
    expect(serialize({ in: "header", name: "X-Filter", schema: { type: "string" } }, "draft review").headerValue).toBe("draft review");
    expect(serialize({ in: "cookie", name: "ids", schema: { type: "array" } }, "a,b").cookieEntries).toEqual([
      ["ids", "a"],
      ["ids", "b"],
    ]);
    expect(serialize({ in: "cookie", name: "meta", explode: true, schema: { type: "object" } }, '{"role":"admin"}').cookieEntries).toEqual([["role", "admin"]]);
  });

  it("honors allowReserved while keeping default query values encoded", () => {
    const value = "quotes/h2g2.txt";
    expect(serialize({ in: "query", name: "file", schema: { type: "string" } }, value).queryEntries).toEqual([["file", "quotes%2Fh2g2.txt"]]);
    expect(serialize({ in: "query", name: "file", allowReserved: true, schema: { type: "string" } }, value).queryEntries).toEqual([["file", "quotes/h2g2.txt"]]);
    expect(serialize({ in: "query", name: "file", allowReserved: true, schema: { type: "string" } }, "a&b#c+d").queryEntries).toEqual([["file", "a%26b%23c%2Bd"]]);
  });

  it("restores encoded query and path arrays and objects without splitting escaped delimiters", () => {
    expect(readOpenApiQueryParameter({ in: "query", name: "status", style: "form", explode: false, schema: { type: "array" } }, "https://api.example.test/items?status=draft%2Creview,active")).toBe('["draft,review","active"]');
    expect(readOpenApiQueryParameter({ in: "query", name: "filter", style: "deepObject", schema: { type: "object" } }, "https://api.example.test/items?filter%5Brole%5D=read%2Fwrite")).toBe('{"role":"read/write"}');
    expect(readOpenApiPathParameter("/items/{id}", "/items/.draft%2Creview.active", { in: "path", name: "id", style: "label", explode: true, schema: { type: "array" } })).toBe('["draft,review","active"]');
    expect(readOpenApiPathParameter("/items/{id}", "/items/;id=first;id=second", { in: "path", name: "id", style: "matrix", explode: true, schema: { type: "array" } })).toBe('["first","second"]');
    expect(readOpenApiPathParameter("/items/{filter}", "/items/.role=admin.active%2Eflag=true", { in: "path", name: "filter", style: "label", explode: true, schema: { type: "object" } })).toBe('{"role":"admin","active.flag":"true"}');
  });

  it("serializes nested referenced values with deepObject bracket paths", () => {
    const result = serializeOpenApiParameter({ in: "query", name: "filter", style: "deepObject", schema: { type: "object" } }, '{"profile":{"active":true},"ids":[1,2]}');
    expect(result).toMatchObject({
      ok: true,
      value: {
        queryEntries: [
          ["filter%5Bprofile%5D%5Bactive%5D", "true"],
          ["filter%5Bids%5D%5B0%5D", "1"],
          ["filter%5Bids%5D%5B1%5D", "2"],
        ],
      },
    });
  });

  it("restores nested deepObject fields and arrays without accepting prototype paths", () => {
    const parameter = { in: "query", name: "filter", style: "deepObject", schema: { type: "object" } };
    expect(JSON.parse(readOpenApiQueryParameter(parameter, new URLSearchParams("filter[profile][name]=fox&filter[ids][0]=1&filter[ids][1]=2&filter[__proto__][bad]=true"))!)).toEqual({ profile: { name: "fox" }, ids: ["1", "2"] });
    expect(({} as Record<string, unknown>).bad).toBeUndefined();
  });

  it("rejects malformed arrays, objects, nested values and unsupported styles", () => {
    expect(serializeOpenApiParameter({ in: "query", name: "ids", schema: { type: "array" } }, "[not-json")).toMatchObject({ ok: false, error: "invalid_array" });
    expect(serializeOpenApiParameter({ in: "query", name: "filter", schema: { type: "object" } }, "role")).toMatchObject({ ok: false, error: "invalid_object" });
    expect(serializeOpenApiParameter({ in: "query", name: "filter", style: "form", schema: { type: "object" } }, '{"nested":{"name":"x"}}')).toMatchObject({ ok: false, error: "nested_value" });
    expect(serializeOpenApiParameter({ in: "path", name: "id", style: "form", schema: { type: "string" } }, "x")).toMatchObject({ ok: false, error: "unsupported_style" });
  });

  it("restores parameters after a server base path and rejects a different operation suffix", () => {
    const parameter = { in: "path", name: "id", style: "matrix", explode: true, schema: { type: "array" } };
    expect(readOpenApiPathParameter("/items/{id}", "/v2/tenant/items/;id=first;id=second", parameter)).toBe('["first","second"]');
    expect(readOpenApiPathParameter("/items/{id}", "/v2/tenant/other/;id=first;id=second", parameter)).toBeUndefined();
  });
});
