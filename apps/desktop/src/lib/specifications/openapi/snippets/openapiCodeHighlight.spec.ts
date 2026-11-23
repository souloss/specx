import { describe, expect, it } from "vitest";
import { highlightOpenApiCode } from "./openapiCodeHighlight";
import { buildCodeSnippet } from "./openapiCodegen";
import { SNIPPET_CLIENTS } from "./openapiSnippetClients";

describe("request code highlighting", () => {
  it.each(SNIPPET_CLIENTS)("highlights $id without changing the copyable source", ({ id }) => {
    const source = buildCodeSnippet({ method: "POST", url: "https://api.example.test/items?q=hello", headers: [], cookies: [], body: '{"name":"fox"}', mediaType: "application/json" }, id);
    const html = highlightOpenApiCode(source, id);
    const element = document.createElement("code");
    element.innerHTML = html;
    expect(element.textContent).toBe(source);
    expect(element.querySelector('[class^="hljs-"]')).not.toBeNull();
  });
  it.each(["javascript", "json", "python", "text"])("escapes markup in %s", (language) => {
    const source = "<script>alert(1)</script><img src=x onerror=alert(2)>";
    const element = document.createElement("code");
    element.innerHTML = highlightOpenApiCode(source, language);
    expect(element.querySelector("script,img")).toBeNull();
    expect(element.textContent).toBe(source);
  });
});
