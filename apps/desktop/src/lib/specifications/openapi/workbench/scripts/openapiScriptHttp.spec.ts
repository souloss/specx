// @vitest-environment node
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { createServer, type Server } from "node:http";
import { createOpenApiScriptHttpHost } from "./openapiScriptHttp";
import { specxApi } from "../../../../backend/client";
import { bytesToBase64 } from "../request/openapiRequestAgent";
let server: Server, baseUrl: string;
const received: Array<{ headers: Record<string, unknown>; method: string; body: string }> = [];
beforeAll(async () => {
  server = createServer(async (request, response) => {
    const chunks: Buffer[] = [];
    for await (const chunk of request) chunks.push(Buffer.from(chunk));
    const body = Buffer.concat(chunks).toString();
    received.push({ headers: request.headers, method: request.method ?? "", body });
    if (request.url === "/slow") {
      setTimeout(() => response.end("late"), 200);
      return;
    }
    response.setHeader("Content-Type", "application/json");
    response.end(JSON.stringify({ method: request.method, body }));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("missing address");
  baseUrl = `http://127.0.0.1:${address.port}`;
});
afterAll(async () => {
  await new Promise<void>((resolve) => server.close(() => resolve()));
});
const host = () => createOpenApiScriptHttpHost({ proxy: "browser", baseUrl, timeoutMs: 5000, followRedirects: true });
describe("script HTTP host", () => {
  it("sends a real HTTP body and explicit headers and returns the response", async () => {
    const result = await host().sendRequest!({ url: "/echo", method: "POST", headers: [["X-Example", "value"]], body: '{"sent":true}', mediaType: "application/json" });
    expect(result.status).toBe(200);
    expect(JSON.parse(result.body)).toEqual({ method: "POST", body: '{"sent":true}' });
    expect(received.at(-1)?.headers["x-example"]).toBe("value");
    expect(received.at(-1)?.headers["cookie"]).toBeUndefined();
  });
  it("rejects non-HTTP URLs and embedded credentials before sending", async () => {
    await expect(host().sendRequest!({ url: "file:///etc/passwd", method: "GET", headers: [] })).rejects.toThrow("script_http_invalid_url");
    await expect(host().sendRequest!({ url: "https://user:password@example.test", method: "GET", headers: [] })).rejects.toThrow("script_http_invalid_url");
  });
  it("aborts in-flight HTTP work with the execution signal", async () => {
    const controller = new AbortController();
    const promise = host().sendRequest!({ url: "/slow", method: "GET", headers: [] }, controller.signal);
    controller.abort();
    await expect(promise).rejects.toMatchObject({ name: "AbortError" });
  });
  it("uses the cloud Agent with the same workspace/service and explicit request bytes", async () => {
    const execute = vi
      .spyOn(specxApi, "executeRequestAgent")
      .mockResolvedValue({ status: 201, statusText: "Created", headers: [["X-Echo", "yes"]], bodyBase64: bytesToBase64(new TextEncoder().encode('{"id":7}')), truncated: false, timing: { totalMs: 10 } } as Awaited<ReturnType<typeof specxApi.executeRequestAgent>>);
    try {
      const agent = createOpenApiScriptHttpHost({ proxy: "agent", baseUrl, workspaceId: "workspace", serviceId: "service", timeoutMs: 5000, followRedirects: true });
      const result = await agent.sendRequest!({ url: "https://example.test/echo", method: "POST", headers: [["X-Example", "value"]], body: "hello" });
      expect(result.body).toBe('{"id":7}');
      expect(result.status).toBe(201);
      expect(execute.mock.calls[0]?.slice(0, 2)).toEqual(["workspace", "service"]);
      expect(execute.mock.calls[0]?.[3]).toMatchObject({ method: "POST", headers: expect.arrayContaining([["x-example", "value"]]), bodyBase64: bytesToBase64(new TextEncoder().encode("hello")) });
    } finally {
      execute.mockRestore();
    }
  });
});
