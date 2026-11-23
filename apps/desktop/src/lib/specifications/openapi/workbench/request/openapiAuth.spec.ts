import { describe, expect, it } from "vitest";
import { applyOpenApiAuth, defaultAuthConfig, emptyAuthInput, isAuthConfig, redactAuthRequest, type OpenApiAuthConfig } from "./openapiAuth";
import { appendWorkbenchHistory } from "../openapiWorkbench";
import { installMemoryStorage } from "@/test/browserStorage";

const schemes = [
  { name: "headerKey", scheme: { type: "apiKey", in: "header", name: "X-Workspace" } },
  { name: "queryKey", scheme: { type: "apiKey", in: "query", name: "access[key]" } },
  { name: "cookieKey", scheme: { type: "apiKey", in: "cookie", name: "session" } },
  { name: "basic", scheme: { type: "http", scheme: "basic" } },
  { name: "bearer", scheme: { type: "http", scheme: "bearer" } },
  { name: "oauth", scheme: { type: "oauth2" } },
];
const requirements = [{ headerKey: [], queryKey: [] }, { basic: [] }, {}];
const apply = (config: OpenApiAuthConfig = defaultAuthConfig(), extra: Partial<Parameters<typeof applyOpenApiAuth>[0]> = {}) =>
  applyOpenApiAuth({ config, schemes, requirements, url: "https://api.test/path?q=a+b&access%5Bkey%5D=old&access%5Bkey%5D=duplicate#section", headers: new Headers(), variables: { headerKey: "header-secret", queryKey: "query-secret" }, ...extra });
const configFor = (scheme: string, input = emptyAuthInput()): OpenApiAuthConfig => ({ mode: "scheme", alternative: 0, scheme, inputs: { [scheme]: input } });

describe("OpenAPI effective request authentication", () => {
  it("applies every AND member and replaces only its query location before the fragment", () => {
    const result = apply();
    expect(result.issues).toEqual([]);
    expect(result.headers.get("X-Workspace")).toBe("header-secret");
    expect(result.url).toBe("https://api.test/path?q=a+b&access%5Bkey%5D=query-secret#section");
    expect(result.targets).toEqual({ headers: ["X-Workspace"], query: ["access[key]"], cookies: [] });
  });
  it("selects exactly one OR alternative, including anonymous and explicit no-auth", () => {
    const result = apply({ ...defaultAuthConfig(), alternative: 1 }, { variables: { USERNAME: "name", PASSWORD: "password" } });
    expect(result.headers.get("authorization")).toBe(`Basic ${btoa("name:password")}`);
    expect(result.headers.has("X-Workspace")).toBe(false);
    expect(apply({ ...defaultAuthConfig(), alternative: 2 }).targets.headers).toEqual([]);
    expect(apply(defaultAuthConfig("none")).issues).toEqual([]);
    expect(apply({ ...defaultAuthConfig(), alternative: 4 }).issues).toEqual([{ code: "invalid", scheme: "" }]);
  });
  it("encodes Unicode Basic credentials as UTF-8 and permits a blank password", () => {
    const config = configFor("basic", { value: "", username: "用户", password: "密:码" });
    const result = apply(config);
    const expected = `Basic ${btoa(String.fromCharCode(...new TextEncoder().encode("用户:密:码")))}`;
    expect(result.headers.get("authorization")).toBe(expected);
    expect(apply(configFor("basic", { value: "", username: "name", password: "" })).headers.get("authorization")).toBe(`Basic ${btoa("name:")}`);
    expect(apply(configFor("basic", { value: "", username: "invalid:name", password: "" })).issues[0]?.code).toBe("missing");
    expect(apply(configFor("basic"), { variables: {} }).issues[0]?.code).toBe("missing");
  });
  it("resolves whole templates and environment aliases, without accepting unresolved or redacted values", () => {
    const config = configFor("headerKey", { ...emptyAuthInput(), value: "{{STAGING_KEY}}" });
    expect(apply(config, { variables: { STAGING_KEY: "staging" } }).headers.get("X-Workspace")).toBe("staging");
    expect(apply(config).issues).toEqual([{ code: "missing", scheme: "headerKey" }]);
    expect(apply(configFor("headerKey", { ...emptyAuthInput(), value: "{{redacted}}" }), { variables: {} }).issues[0]?.code).toBe("missing");
    expect(apply(configFor("headerKey"), { variables: { HEADERKEY_KEY: "alias" } }).headers.get("X-Workspace")).toBe("alias");
  });
  it("preserves explicit headers and flags duplicate generated destinations", () => {
    const headers = new Headers({ "X-Workspace": "explicit" });
    const result = apply(defaultAuthConfig(), { headers, variables: { queryKey: "query" } });
    expect(result.issues).toEqual([]);
    expect(result.headers.get("X-Workspace")).toBe("explicit");
    expect(headers.get("X-Workspace")).toBe("explicit");
    const collision = apply(defaultAuthConfig(), { requirements: [{ basic: [], bearer: [] }], variables: { USERNAME: "user", PASSWORD: "pw", TOKEN: "token" } });
    expect(collision.issues).toEqual([{ code: "conflict", scheme: "bearer" }]);
  });
  it("does not require credentials for unchecked generated headers, while preserving query authentication", () => {
    const result = apply(defaultAuthConfig(), { requirements: [{ bearer: [], headerKey: [], cookieKey: [], queryKey: [] }], disabledHeaders: ["AUTHORIZATION", "x-workspace", "Cookie"], variables: { queryKey: "query-secret" } });
    expect(result.issues).toEqual([]);
    expect([...result.headers.entries()]).toEqual([]);
    expect(new URL(result.url).searchParams.get("access[key]")).toBe("query-secret");
    expect(apply(defaultAuthConfig(), { disabledHeaders: ["x-workspace"], variables: {} }).issues).toEqual([{ code: "missing", scheme: "queryKey" }]);
    expect(apply(configFor("bearer"), { variables: {} }).issues).toEqual([{ code: "missing", scheme: "bearer" }]);
  });
  it("supports cookie API keys, OAuth memory tokens and unsupported-scheme errors", () => {
    expect(apply(configFor("cookieKey"), { variables: { API_KEY: "a+b" }, headers: new Headers({ Cookie: "other=one; session=old" }) }).headers.get("cookie")).toBe("other=one; session=a%2Bb");
    expect(apply(configFor("oauth"), { tokens: { oauth: "Bearer memory-token" } }).headers.get("authorization")).toBe("Bearer memory-token");
    expect(apply(configFor("unknown")).issues).toEqual([{ code: "unsupported", scheme: "unknown" }]);
    expect(apply(configFor("bearer"), { schemes: [{ name: "bearer", scheme: { type: "http", scheme: "digest" } }] }).issues[0]?.code).toBe("unsupported");
  });
  it("generates previews from safe templates without embedding literal credentials", () => {
    const result = apply(configFor("headerKey", { ...emptyAuthInput(), value: "literal-secret" }), { preview: true });
    expect(result.headers.get("X-Workspace")).toBe("{{API_KEY}}");
    const template = apply(configFor("queryKey", { ...emptyAuthInput(), value: "{{ENV_KEY}}" }), { preview: true });
    expect(new URL(template.url).searchParams.get("access[key]")).toBe("{{ENV_KEY}}");
  });
  it("redacts custom auth locations before local history persistence", () => {
    installMemoryStorage().clear();
    const result = apply();
    const request = redactAuthRequest({ method: "GET", url: result.url, headers: [...result.headers.entries()].map(([name, value]) => ({ name, value, enabled: true })), cookies: [], mediaType: "", body: "" }, result.targets);
    const history = appendWorkbenchHistory("auth-history", { operationKey: "GET:/path", operationLabel: "GET /path", request, response: { status: 200, statusText: "OK", duration: 1 } });
    expect(JSON.stringify(history)).not.toContain("header-secret");
    expect(JSON.stringify(history)).not.toContain("query-secret");
    expect(new URL(history[0]!.request.url).searchParams.get("access[key]")).toBe("{{redacted}}");
  });
  it("rejects malformed persisted configs at the client boundary", () => {
    const valid = configFor("basic");
    expect(isAuthConfig(valid)).toBe(true);
    for (const config of [
      { ...valid, scheme: undefined },
      { ...valid, alternative: 64 },
      { ...valid, inputs: { "": emptyAuthInput() } },
      { ...valid, inputs: { basic: { ...emptyAuthInput(), password: "bad\nvalue" } } },
      { ...valid, extra: true },
    ])
      expect(isAuthConfig(config)).toBe(false);
  });
});
