import { beforeEach, describe, expect, it } from "vitest";
import { installMemoryStorage } from "@/test/browserStorage";
import { buildCodeSnippet, buildCurlCommand, clearWorkbenchHistory, loadWorkbenchHistory, parseCurlCommand, parseWorkbenchBulkEdit, formatWorkbenchBulkEdit, appendWorkbenchHistory } from "./openapiWorkbench";

const request = {
  method: "POST",
  url: "https://api.example.test/users?q=one",
  headers: [
    { name: "content-type", value: "application/json", enabled: true },
    { name: "Authorization", value: "secret-token", enabled: true },
  ],
  cookies: [{ name: "session-token", value: "session-secret", enabled: true }],
  body: '{"name":"Ada","token":"should-not-persist"}',
  mediaType: "application/json",
};

describe("OpenAPI workbench persistence and snippets", () => {
  const storage = installMemoryStorage();
  beforeEach(() => storage.clear());

  it("parses common cURL flags and defaults data requests to POST", () => {
    expect(parseCurlCommand("curl --url 'https://api.example.test/users' -H 'X-Trace: one' -d '{\"name\":\"Ada\"}' --cookie 'a=b; c=d'")).toEqual({
      url: "https://api.example.test/users",
      method: "POST",
      headers: [{ name: "X-Trace", value: "one", enabled: true }],
      body: '{"name":"Ada"}',
      cookies: [
        { name: "a", value: "b", enabled: true },
        { name: "c", value: "d", enabled: true },
      ],
    });
  });

  it("parses and formats row-based bulk edits while preserving disabled rows and colons in values", () => {
    const parsed = parseWorkbenchBulkEdit("X-Trace: one:two\r\n# Authorization: token\r\nquery: value\r\n");
    expect(parsed).toEqual({
      rows: [
        { name: "X-Trace", value: "one:two", enabled: true },
        { name: "Authorization", value: "token", enabled: false },
        { name: "query", value: "value", enabled: true },
      ],
    });
    expect(formatWorkbenchBulkEdit(parsed.rows)).toBe("X-Trace: one:two\n# Authorization: token\nquery: value");
    expect(parseWorkbenchBulkEdit("valid: row\ninvalid row")).toEqual({ rows: [{ name: "valid", value: "row", enabled: true }], error: { line: 2 } });
  });

  it("round trips standard five-column CSV including quoted commas, quotes and newlines", () => {
    const rows = [
      { enabled: false, name: "a,b", value: 'say "hello"\nnext', type: "string", description: "one,two" },
      { enabled: true, name: "empty", value: "", type: "integer", description: "" },
    ];
    const text = formatWorkbenchBulkEdit(rows, "comma");
    expect(text).toContain('"say ""hello""\nnext"');
    expect(parseWorkbenchBulkEdit(text.replaceAll("\n", "\r\n"), "comma")).toEqual({ rows: [{ ...rows[0], value: 'say "hello"\r\nnext' }, rows[1]] });
    expect(parseWorkbenchBulkEdit('true,name,"unterminated', "comma").error?.line).toBe(1);
    expect(parseWorkbenchBulkEdit("true,name,value,string,description,extra", "comma").error?.line).toBe(1);
  });

  it("builds cURL and language snippets", () => {
    expect(buildCurlCommand(request)).toContain("--request 'POST'");
    expect(buildCurlCommand(request)).toContain("--header 'content-type: application/json'");
    expect(buildCodeSnippet(request, "fetch")).toContain('fetch("https://api.example.test/users?q=one"');
    expect(buildCodeSnippet(request, "python")).toContain("requests.request");
    expect(buildCodeSnippet(request, "axios")).toContain("axios({");
    expect(buildCodeSnippet(request, "java")).toContain("HttpRequest.newBuilder");
    expect(buildCodeSnippet(request, "go")).toContain("http.NewRequest");
    expect(buildCodeSnippet(request, "php")).toContain("curl_init");
    expect(buildCodeSnippet(request, "ruby")).toContain("Net::HTTP");
    expect(buildCodeSnippet(request, "csharp")).toContain("HttpRequestMessage");
  });

  it("redacts credentials before writing history", () => {
    appendWorkbenchHistory("svc", { operationKey: "POST:/users", operationLabel: "POST /users", request, response: { status: 201, statusText: "Created", duration: 8 } });
    const history = loadWorkbenchHistory("svc");
    expect(history[0]?.request.headers.find((header) => header.name === "Authorization")?.value).toBe("{{redacted}}");
    expect(history[0]?.request.cookies[0]?.value).toBe("{{redacted}}");
    expect(history[0]?.request.body).toContain("{{redacted}}");
    clearWorkbenchHistory("svc");
    expect(loadWorkbenchHistory("svc")).toEqual([]);
  });
});
