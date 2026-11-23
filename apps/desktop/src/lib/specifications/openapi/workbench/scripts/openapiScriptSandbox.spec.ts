import { afterEach, describe, expect, it, vi } from "vitest";
import { OPENAPI_SCRIPT_MAX_LENGTH, OPENAPI_SCRIPT_TIMEOUT_MS, runOpenApiScript, runOpenApiScriptSequence, scriptVariables, validateOpenApiScript } from "./openapiScriptSandbox";
import type { OpenApiScriptInput, OpenApiScriptOutput } from "./openapiScriptSandbox";

class ScriptWorker extends EventTarget {
  static instances: ScriptWorker[] = [];
  terminated = false;
  messages: unknown[] = [];
  constructor() {
    super();
    ScriptWorker.instances.push(this);
  }
  postMessage(value: unknown) {
    this.messages.push(value);
  }
  terminate() {
    this.terminated = true;
  }
  reply(value: unknown) {
    this.dispatchEvent(new MessageEvent("message", { data: value }));
  }
}
const input: OpenApiScriptInput = { kind: "preRequest", script: 'console.log("hello");', variables: { region: "west", accessToken: "excluded" }, request: { method: "GET", url: "https://example.test", headers: [], cookies: [], body: "", mediaType: "" } };
const output: OpenApiScriptOutput = { request: input.request, variables: { region: "west" }, logs: ["hello"], tests: [] };
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  ScriptWorker.instances = [];
});

describe("OpenAPI script sandbox policy", () => {
  it("keeps ordinary environment variables and excludes credential-like names", () => {
    expect(scriptVariables({ baseUrl: "https://api.test", accessToken: "secret", API_KEY: "secret", tenant: "demo" })).toEqual({ baseUrl: "https://api.test", tenant: "demo" });
  });

  it("enforces a bounded script size without rejecting ordinary JavaScript identifiers", () => {
    expect(validateOpenApiScript("const windowSize = 1;")).toBeNull();
    expect(validateOpenApiScript("x".repeat(OPENAPI_SCRIPT_MAX_LENGTH + 1))).toBe("script_too_large");
  });
  it("rejects an already cancelled run before creating a worker", async () => {
    vi.stubGlobal("Worker", ScriptWorker);
    await expect(runOpenApiScript(input, AbortSignal.abort())).rejects.toMatchObject({ name: "AbortError" });
    expect(ScriptWorker.instances).toHaveLength(0);
  });
  it("starts the execution timeout after cold loading has completed", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("Worker", ScriptWorker);
    const promise = runOpenApiScript(input);
    const worker = ScriptWorker.instances[0]!;
    await vi.advanceTimersByTimeAsync(3000);
    expect(worker.terminated).toBe(false);
    worker.reply({ ready: true });
    const assertion = expect(promise).rejects.toThrow("script_timeout");
    await vi.advanceTimersByTimeAsync(OPENAPI_SCRIPT_TIMEOUT_MS + 251);
    await assertion;
    expect(worker.terminated).toBe(true);
  });
  it("bridges an HTTP child request and cleans up the worker on completion", async () => {
    vi.stubGlobal("Worker", ScriptWorker);
    const response = { status: 200, statusText: "OK", body: "ok", headers: [], duration: 1, truncated: false };
    const sendRequest = vi.fn().mockResolvedValue(response);
    const promise = runOpenApiScript(input, undefined, { sendRequest });
    const worker = ScriptWorker.instances[0]!;
    expect(worker.messages[0]).toMatchObject({ variables: { region: "west" } });
    expect((worker.messages[0] as OpenApiScriptInput).variables).not.toHaveProperty("accessToken");
    const request = { method: "GET", url: "https://example.test", headers: [] };
    worker.reply({ rpc: 1, request });
    await Promise.resolve();
    expect(worker.messages[1]).toEqual({ rpc: 1, response });
    worker.reply(output);
    await expect(promise).resolves.toEqual(output);
    expect(worker.terminated).toBe(true);
    expect(sendRequest.mock.calls[0]?.[1].aborted).toBe(true);
  });
  it("cancels child requests and the worker with the parent request", async () => {
    vi.stubGlobal("Worker", ScriptWorker);
    let childSignal: AbortSignal | undefined;
    const controller = new AbortController();
    const promise = runOpenApiScript(input, controller.signal, {
      sendRequest: (_request, signal) => {
        childSignal = signal;
        return new Promise(() => {});
      },
    });
    const worker = ScriptWorker.instances[0]!;
    worker.reply({ rpc: 1, request: { method: "GET", url: "https://example.test", headers: [] } });
    controller.abort();
    await expect(promise).rejects.toMatchObject({ name: "AbortError" });
    expect(worker.terminated).toBe(true);
    expect(childSignal?.aborted).toBe(true);
  });
  it("reports the failing step and retains output from preceding steps", async () => {
    vi.stubGlobal("Worker", ScriptWorker);
    const promise = runOpenApiScriptSequence({ ...input, scripts: [input.script, 'throw new Error("broken");'] });
    ScriptWorker.instances[0]!.reply(output);
    await Promise.resolve();
    await Promise.resolve();
    ScriptWorker.instances[1]!.reply({ error: "broken", output: { ...output, logs: ["step two"] } });
    await expect(promise).rejects.toMatchObject({ step: 2, message: "broken", output: { logs: ["hello", "step two"] } });
  });
});
