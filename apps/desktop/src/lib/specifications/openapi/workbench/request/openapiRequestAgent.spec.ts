import { describe, expect, it, vi } from "vitest";
import { selectedRequestProxy, serializeAgentRequest, bytesToBase64, base64ToBytes, executeAgentRequest } from "./openapiRequestAgent";
import { specxApi } from "@/lib/backend/client";
vi.mock("@/lib/backend/client", () => ({ specxApi: { executeRequestAgent: vi.fn(), cancelRequestAgent: vi.fn(async () => {}) } }));
describe("request Agent transport", () => {
  it("honors environment overrides and defaults auto to the built-in Agent", () => {
    expect(selectedRequestProxy("auto")).toBe("agent");
    expect(selectedRequestProxy("browser", "agent")).toBe("agent");
    expect(selectedRequestProxy("agent", "browser")).toBe("browser");
    expect(selectedRequestProxy("browser", "inherit")).toBe("browser");
  });
  it("serializes binary bytes and preserves request cookies", async () => {
    const request = await serializeAgentRequest({ method: "POST", url: "https://example.com", headers: new Headers({ Cookie: "a=1" }), body: new Uint8Array([0, 255, 7]), timeoutMs: 2000, followRedirects: false });
    expect(base64ToBytes(request.bodyBase64)).toEqual(new Uint8Array([0, 255, 7]));
    expect(request.headers).toContainEqual(["cookie", "a=1"]);
    const bytes = new Uint8Array(32 * 1024).fill(255);
    expect(base64ToBytes(bytesToBase64(bytes))).toEqual(bytes);
  });
  it("retains explicit headers when the browser Request guard removes forbidden headers", async () => {
    const NativeRequest = globalThis.Request;
    vi.stubGlobal(
      "Request",
      class extends NativeRequest {
        constructor(input: RequestInfo | URL, init?: RequestInit) {
          super(input, init);
          this.headers.delete("cookie");
        }
      },
    );
    try {
      const request = await serializeAgentRequest({ method: "POST", url: "https://example.com", headers: new Headers({ Cookie: "session=agent-cookie", "X-Trace": "one" }), body: "body", timeoutMs: 1000, followRedirects: true });
      expect(request.headers).toContainEqual(["cookie", "session=agent-cookie"]);
      expect(request.headers).toContainEqual(["x-trace", "one"]);
    } finally {
      vi.stubGlobal("Request", NativeRequest);
    }
  });
  it("suppresses disabled generated headers after platform body serialization", async () => {
    const request = await serializeAgentRequest({ method: "POST", url: "https://example.com", headers: new Headers({ Authorization: "Bearer secret" }), body: new FormData(), disabledHeaders: ["Content-Type", "Authorization", "Accept"], timeoutMs: 1000, followRedirects: true });
    expect(request.headers).toEqual([]);
    expect(request.disabledHeaders).toEqual(["content-type", "authorization", "accept"]);
    expect(base64ToBytes(request.bodyBase64).length).toBeGreaterThan(0);
  });
  it("cancels the server request on abort and never returns a late result", async () => {
    const stop = new AbortController();
    let complete!: (value: never) => void;
    vi.mocked(specxApi.executeRequestAgent).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const pending = executeAgentRequest("workspace", "service", { method: "GET", url: "https://example.com", headers: [], bodyBase64: "", timeoutMs: 1000, followRedirects: true }, stop.signal);
    stop.abort();
    complete({} as never);
    await expect(pending).rejects.toThrow();
    expect(specxApi.cancelRequestAgent).toHaveBeenCalledWith("workspace", "service", expect.any(String));
  });
});
