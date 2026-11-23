import { describe, expect, it } from "vitest";
import { parseCurlImport } from "./openapiCurl";
import { buildCurlCommand } from "../../snippets/openapiCodegen";

function parse(source: string) {
  const result = parseCurlImport(source);
  if ("error" in result) throw new Error(JSON.stringify(result.error));
  return result;
}

describe("cURL HTTP request import", () => {
  it("defaults a request without a body to GET", () => {
    expect(parse("curl https://example.test").request).toMatchObject({ method: "GET", body: "", mediaType: "" });
  });

  it("preserves POSIX single quotes, backslashes, adjacent fragments and empty data", () => {
    const result = parse(String.raw`curl 'https://example.test' -H 'X-Name: O'\''Brien' --data-raw 'C:\new\file'" and "''`);
    expect(result.request.headers[0]?.value).toBe("O'Brien");
    expect(result.request.body).toBe(String.raw`C:\new\file and `);
    expect(parse("curl https://example.test -d ''").request.method).toBe("POST");
  });

  it("supports line continuation, comments, ANSI quoting and double-quoted escapes", () => {
    const result = parse("curl.exe \\\n'https://example.test' \\\r\n-H \"X-Path: C:\\new\" --data-raw $'中文\\n\\t\\u0041' # comment\n");
    expect(result.request.body).toBe("中文\n\tA");
    expect(result.request.headers[0]?.value).toBe(String.raw`C:\new`);
  });

  it("handles attached, clustered and long options with repeated data", () => {
    const result = parse("curl -sSXPUT https://example.test -H'X-Test: yes' --data=a=1 -db=2");
    expect(result.request).toMatchObject({ method: "PUT", body: "a=1&b=2", mediaType: "application/x-www-form-urlencoded" });
    expect(result.warnings).toEqual(["network"]);
  });

  it("appends encoded GET and url-query values before a fragment", () => {
    const result = parse("curl 'https://example.test/path?start=1#section' -G --data-urlencode 'q=a + 中文' --url-query 'x=one/two' --url-query '+raw=a%2Bb'");
    const url = new URL(result.request.url);
    expect(url.searchParams.get("q")).toBe("a + 中文");
    expect(url.searchParams.get("x")).toBe("one/two");
    expect(url.searchParams.get("raw")).toBe("a+b");
    expect(url.hash).toBe("#section");
    expect(result.request.body).toBe("");
    expect(result.request.method).toBe("GET");
  });

  it("sets JSON media and Accept without replacing explicit headers", () => {
    expect(parse(`curl https://example.test --json '{"percent":"100%"}'`).request).toMatchObject({ method: "POST", body: '{"percent":"100%"}', mediaType: "application/json", headers: [{ name: "Accept", value: "application/json", enabled: true }] });
    expect(parse(`curl https://example.test --json '{}' -H 'Accept: custom' -H 'Content-Type: application/problem+json'`).request).toMatchObject({
      mediaType: "application/problem+json",
      headers: [
        { name: "Accept", value: "custom" },
        { name: "Content-Type", value: "application/problem+json" },
      ],
    });
  });

  it("imports UTF-8 basic and bearer authorization and strips URL credentials", () => {
    const basic = parse("curl https://example.test -u '用户:密钥'").request.headers[0]?.value;
    expect(basic).toBe(`Basic ${btoa(String.fromCharCode(...new TextEncoder().encode("用户:密钥")))}`);
    expect(parse("curl https://example.test --oauth2-bearer token").request.headers[0]?.value).toBe("Bearer token");
    const url = parse("curl 'https://u:p@example.test/path'").request;
    expect(url.url).toBe("https://example.test/path");
    expect(url.headers[0]?.value).toBe(`Basic ${btoa("u:p")}`);
  });

  it("imports cookies without losing equals signs or encoding them again", () => {
    const request = parse("curl https://example.test -b 'a=x%2By; session=a=b' -H 'Cookie: theme=dark'").request;
    expect(request.cookies.map(({ name, value }) => [name, value])).toEqual([
      ["a", "x%2By"],
      ["session", "a=b"],
      ["theme", "dark"],
    ]);
    expect(request.headers).toEqual([]);
  });

  it("preserves duplicate literal multipart values and defers file selection", () => {
    const result = parse("curl https://example.test --form-string 'tag=a+b' --form-string 'tag=@literal;type=text/plain' -F 'file=@/tmp/local.txt;type=text/plain;filename=upload.txt' -H 'Content-Type: multipart/form-data; boundary=stale'");
    expect(Array.from(new URLSearchParams(result.request.body))).toEqual([
      ["tag", "a+b"],
      ["tag", "@literal;type=text/plain"],
    ]);
    expect(result.request.mediaType).toBe("multipart/form-data");
    expect(result.request.headers).toEqual([]);
    expect(result.formFiles).toEqual([{ name: "file", path: "/tmp/local.txt", contentType: "text/plain", fileName: "upload.txt" }]);
    expect(result.warnings).toEqual(["files"]);
  });

  it("handles cURL's quoted multipart paths and attributes separately from shell words", () => {
    const command = buildCurlCommand({ method: "POST", url: "https://example.test", headers: [], cookies: [], mediaType: "multipart/form-data", body: "" }, { form: [{ name: "file", file: { path: 'local,semi;quoted".bin', fileName: "renamed;file.bin", contentType: "application/custom" } }] });
    expect(parse(command).formFiles[0]).toEqual({ name: "file", path: 'local,semi;quoted".bin', fileName: "renamed;file.bin", contentType: "application/custom" });
  });

  it("imports binary body paths as hints without reading files", () => {
    expect(parse("curl https://example.test --data-binary '@/not/a/real/file.bin' -H 'Content-Type: application/octet-stream'")).toMatchObject({ bodyFile: { path: "/not/a/real/file.bin" }, request: { body: "", mediaType: "application/octet-stream" }, warnings: ["files"] });
  });

  it.each([
    ["curl https://example.test --proxy http://proxy.test", "option"],
    ["curl https://example.test -X", "value"],
    ["curl https://example.test --max-time nope", "value"],
    ["curl 'https://example.test", "syntax"],
    ["curl https://example.test; touch /tmp/no", "shell"],
    ["curl https://example.test --data-raw $(whoami)", "shell"],
    ["curl https://example.test --data-raw `whoami`", "shell"],
    ["curl https://example.test | cat", "shell"],
    ["curl file:///etc/passwd", "url"],
    ["curl https://example.test https://other.test", "multiple"],
    ["curl https://example.test -d a=b -F x=y", "conflict"],
    ["curl https://example.test -X GET -d a=b", "conflict"],
    ["curl https://example.test -I -F x=y", "conflict"],
    ["curl https://example.test --data-binary @-", "conflict"],
    ["curl https://example.test --data-urlencode a@/tmp/local", "option"],
  ])("reports unsupported or malformed command: %s", (source, code) => {
    expect(parseCurlImport(source)).toMatchObject({ error: { code } });
  });

  it("round trips generated cURL with hostile literals and multipart metadata", () => {
    const request = { method: "POST", url: "https://example.test/path?a=b", headers: [{ name: "X-Literal", value: "'$(no)`x`", enabled: true }], cookies: [{ name: "session", value: "a%2Bb", enabled: true }], body: '{"value":"O\'Brien\\n中文"}', mediaType: "application/json" };
    expect(parse(buildCurlCommand(request)).request).toEqual({ ...request, headers: [...request.headers, { name: "Content-Type", value: request.mediaType, enabled: true }] });
    const multipart = parse(
      buildCurlCommand(
        { ...request, mediaType: "multipart/form-data" },
        {
          form: [
            { name: "tag", value: "@literal;type=text/plain" },
            { name: "file", file: { path: "upload.bin", fileName: "sample.bin", contentType: "application/custom" } },
          ],
        },
      ),
    );
    expect(multipart.formFiles[0]).toEqual({ name: "file", path: "upload.bin", fileName: "sample.bin", contentType: "application/custom" });
    expect(new URLSearchParams(multipart.request.body).get("tag")).toBe("@literal;type=text/plain");
  });
});
