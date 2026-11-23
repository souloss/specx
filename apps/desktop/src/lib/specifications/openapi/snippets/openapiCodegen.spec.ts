// @vitest-environment node
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createServer, type Server } from "node:http";
import { execFile, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildCodeSnippet, type SnippetOptions } from "./openapiCodegen";
import type { WorkbenchRequest } from "../workbench/openapiWorkbench";

const exec = promisify(execFile);
const received: Array<{ method?: string; headers: Record<string, string | string[] | undefined>; body: Buffer }> = [];
let server: Server;
let url: string;
let directory: string;

beforeAll(async () => {
  directory = await mkdtemp(join(tmpdir(), "specx-codegen-"));
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    received.push({ method: req.method, headers: req.headers, body: Buffer.concat(chunks) });
    if (req.url === "/slow") await new Promise((resolve) => setTimeout(resolve, 300));
    res.end("received");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("No HTTP listener");
  url = `http://127.0.0.1:${address.port}`;
});
afterAll(async () => {
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  await rm(directory, { recursive: true, force: true });
});
function request(overrides: Partial<WorkbenchRequest> = {}): WorkbenchRequest {
  return {
    method: "PATCH",
    url,
    headers: [
      { name: "X-Literal", value: "O'Brien $x #{raise 1}", enabled: true },
      { name: "Disabled", value: "no", enabled: false },
    ],
    cookies: [
      { name: "session", value: "a%2Bb", enabled: true },
      { name: "disabled", value: "no", enabled: false },
    ],
    mediaType: "application/json",
    body: JSON.stringify({ value: "中文🦊\n'\"\\$x #{raise 1}\u2028\u0000\u000b\\/" }),
    ...overrides,
  };
}
async function execute(value: WorkbenchRequest, options: SnippetOptions = {}) {
  const code = buildCodeSnippet(value, "fetch", options);
  return exec(process.execPath, ["--input-type=module", "--eval", code], { timeout: 5000 });
}

describe("executable request code generation", () => {
  it("sends literal UTF-8 bytes, headers and cookies without evaluating values", async () => {
    const value = request();
    await execute(value);
    const actual = received.at(-1)!;
    expect(actual.method).toBe("PATCH");
    expect(actual.body.toString()).toBe(value.body);
    expect(actual.headers["content-type"]).toBe("application/json");
    expect(actual.headers["x-literal"]).toBe(value.headers[0]?.value);
    expect(actual.headers.cookie).toBe("session=a%2Bb");
    expect(actual.headers.disabled).toBeUndefined();
  });

  it("sends actual multipart files, duplicate literal fields and upload metadata", async () => {
    const path = join(directory, "local.bin");
    const bytes = Buffer.from([0, 1, 255, 13, 10]);
    await writeFile(path, bytes);
    await execute(request({ mediaType: "multipart/form-data", headers: [{ name: "Content-Type", value: "multipart/form-data; boundary=old", enabled: true }] }), {
      form: [
        { name: "tag", value: "first + 中文" },
        { name: "tag", value: "@literal;type=text/plain" },
        { name: "file", file: { path, fileName: "renamed.bin", contentType: "application/custom" } },
      ],
    });
    const actual = received.at(-1)!;
    const contentType = String(actual.headers["content-type"]);
    expect(contentType).toContain("multipart/form-data; boundary=");
    expect(contentType).not.toContain("boundary=old");
    const form = await new Response(new Uint8Array(actual.body), { headers: { "content-type": contentType } }).formData();
    expect(form.getAll("tag")).toEqual(["first + 中文", "@literal;type=text/plain"]);
    const file = form.get("file") as File;
    expect(file.name).toBe("renamed.bin");
    expect(file.type).toBe("application/custom");
    expect(Buffer.from(await file.arrayBuffer())).toEqual(bytes);
  });

  it("sends binary files without converting them into text", async () => {
    const path = join(directory, "binary.bin");
    const bytes = Buffer.from([0, 255, 192, 13, 10]);
    await writeFile(path, bytes);
    await execute(request({ mediaType: "application/octet-stream" }), { file: { path } });
    expect(received.at(-1)!.body).toEqual(bytes);
    expect(received.at(-1)!.headers["content-type"]).toBe("application/octet-stream");
  });

  it("omits request bodies for GET and HEAD", async () => {
    await execute(request({ method: "GET" }), { file: { path: "/does/not/exist" } });
    expect(received.at(-1)!.method).toBe("GET");
    expect(received.at(-1)!.body.length).toBe(0);
  });

  it("generates a valid empty multipart body instead of dropping its media type", async () => {
    await execute(request({ mediaType: "multipart/form-data", body: "" }), { form: [] });
    const actual = received.at(-1)!;
    const form = await new Response(new Uint8Array(actual.body), { headers: { "content-type": String(actual.headers["content-type"]) } }).formData();
    expect(Array.from(form)).toEqual([]);
  });

  it.skipIf(spawnSync("java", ["--version"], { stdio: "ignore" }).status !== 0)(
    "executes Java strings without rewriting JSON Unicode escapes",
    async () => {
      const value = request();
      const path = join(directory, "SpecXRequest.java");
      await writeFile(path, buildCodeSnippet(value, "java"));
      await exec("java", [path], { timeout: 20000 });
      expect(received.at(-1)!.body.toString()).toBe(value.body);
      expect(received.at(-1)!.headers.cookie).toBe("session=a%2Bb");
    },
    30000,
  );

  it.skipIf(spawnSync("curl", ["--version"], { stdio: "ignore" }).status !== 0)("executes cURL multipart quoting for paths containing commas, semicolons and quotes", async () => {
    const path = join(directory, 'local,semi;quoted".bin');
    const bytes = Buffer.from([0, 255, 13, 10]);
    await writeFile(path, bytes);
    const command = buildCodeSnippet(request({ mediaType: "multipart/form-data" }), "curl", { form: [{ name: "file", file: { path, fileName: "renamed.bin", contentType: "application/custom" } }] });
    await exec("sh", ["-c", command], { timeout: 5000 });
    const actual = received.at(-1)!;
    const form = await new Response(new Uint8Array(actual.body), { headers: { "content-type": String(actual.headers["content-type"]) } }).formData();
    const file = form.get("file") as File;
    expect(file.name).toBe("renamed.bin");
    expect(file.type).toBe("application/custom");
    expect(Buffer.from(await file.arrayBuffer())).toEqual(bytes);
  });

  it("enforces the generated timeout", async () => {
    await expect(execute(request({ url: `${url}/slow` }), { timeoutMs: 50 })).rejects.toThrow(/TimeoutError/);
  });

  describe.each(["node", "httpie", "wget"] as const)("%s client", (language) => {
    const binary = language === "node" ? process.execPath : language === "httpie" ? "http" : "wget";
    it.skipIf(spawnSync(binary, ["--version"], { stdio: "ignore" }).status !== 0)(
      "sends literal UTF-8 and multipart/binary bodies",
      async () => {
        const path = join(directory, 'literal,semi;quote".bin');
        const bytes = Buffer.from([0, 255, 13, 10]);
        await writeFile(path, bytes);
        async function run(value: WorkbenchRequest, options: SnippetOptions = {}) {
          const code = buildCodeSnippet(value, language, options);
          if (language === "node") await exec(process.execPath, ["--input-type=module", "--eval", code], { timeout: 5000 });
          else await exec("sh", ["-c", code], { timeout: 5000 });
          return received.at(-1)!;
        }
        const value = request();
        expect((await run(value)).body.toString()).toBe(value.body);
        const actual = await run(request({ mediaType: "multipart/form-data" }), {
          form: [
            { name: "tag", value: "first + 中文" },
            { name: "tag", value: "@literal $x" },
            { name: "file", file: { path, fileName: "renamed.bin", contentType: "application/custom" } },
          ],
        });
        const form = await new Response(new Uint8Array(actual.body), { headers: { "content-type": String(actual.headers["content-type"]) } }).formData();
        expect(form.getAll("tag")).toEqual(["first + 中文", "@literal $x"]);
        const file = form.get("file") as File;
        expect(file.name).toBe("renamed.bin");
        expect(file.type).toBe("application/custom");
        expect(Buffer.from(await file.arrayBuffer())).toEqual(bytes);
        expect((await run(request({ mediaType: "application/octet-stream" }), { file: { path } })).body).toEqual(bytes);
        expect((await run(request({ method: "GET" }))).body.byteLength).toBe(0);
      },
      20000,
    );
  });
});
