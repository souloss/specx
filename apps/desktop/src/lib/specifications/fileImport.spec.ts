import { describe, expect, it } from "vitest";
import { readSpecificationFile } from "./fileImport";

describe("specification file import", () => {
  it("imports Markdown content and derives its title from the file name", async () => {
    const result = await readSpecificationFile(new File(["# Getting started"], "getting-started.md"), "markdown");
    expect(result).toMatchObject({ fileName: "getting-started.md", title: "getting-started", content: "# Getting started" });
  });

  it("imports OpenAPI JSON and uses the document title", async () => {
    const result = await readSpecificationFile(new File(['{"openapi":"3.1.0","info":{"title":"Accounts API","version":"1"},"paths":{}}'], "accounts.json"), "openapi");
    expect(result).toMatchObject({ fileName: "accounts.json", title: "Accounts API", format: "json" });
  });

  it("rejects files with a mismatched extension or invalid OpenAPI syntax", async () => {
    await expect(readSpecificationFile(new File(["# Notes"], "notes.md"), "openapi")).rejects.toThrow("unsupported_type");
    await expect(readSpecificationFile(new File(["openapi: ["], "api.yaml"), "openapi")).rejects.toThrow("invalid_openapi");
  });

  it("enforces the file size limit", async () => {
    const file = new File([new Uint8Array(5 * 1024 * 1024 + 1)], "large.md");
    await expect(readSpecificationFile(file, "markdown")).rejects.toThrow("too_large");
  });
});
