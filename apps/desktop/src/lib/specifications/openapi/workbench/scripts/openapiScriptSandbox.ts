import type { WorkbenchRequest, WorkbenchResponse } from "../openapiWorkbench";

export type OpenApiScriptKind = "preRequest" | "postResponse";
export type OpenApiScriptInput = {
  kind: OpenApiScriptKind;
  locale?: "zh_CN" | "en";
  script: string;
  request: WorkbenchRequest;
  response?: WorkbenchResponse;
  variables: Record<string, string>;
  localVariables?: Record<string, string>;
  scopes?: { environment: Record<string, string>; globals: Record<string, string> };
  info?: { requestName: string; requestId: string; baseUrl?: string };
  messages?: { requestAlreadySent: string };
};
export type OpenApiScriptTest = { name: string; passed: boolean; error?: string };
export type OpenApiScriptHttpRequest = { url: string; method: string; headers: Array<[string, string]>; body?: string; mediaType?: string };
export type OpenApiScriptHost = { sendRequest?: (request: OpenApiScriptHttpRequest, signal?: AbortSignal) => Promise<WorkbenchResponse> };
export type OpenApiScriptOutput = {
  request: WorkbenchRequest;
  variables: Record<string, string>;
  localVariables?: Record<string, string>;
  logs: string[];
  tests: OpenApiScriptTest[];
  scopes?: { environment: Record<string, string>; globals: Record<string, string> };
  response?: WorkbenchResponse;
  visualization?: string;
};

export class OpenApiScriptError extends Error {
  constructor(
    public readonly kind: OpenApiScriptKind,
    public readonly step: number,
    message: string,
    public readonly output?: OpenApiScriptOutput,
  ) {
    super(message);
    this.name = "OpenApiScriptError";
  }
}

export async function runOpenApiScriptSequence(input: Omit<OpenApiScriptInput, "script"> & { scripts: string[] }, signal?: AbortSignal, host?: OpenApiScriptHost): Promise<OpenApiScriptOutput> {
  let output: OpenApiScriptOutput = { request: input.request, variables: input.variables, scopes: input.scopes, localVariables: input.localVariables, response: input.response, logs: [], tests: [] };
  for (const [index, script] of input.scripts.entries()) {
    if (!script.trim()) continue;
    try {
      const result = await runOpenApiScript({ ...input, script, request: output.request, variables: output.variables, scopes: output.scopes, localVariables: output.localVariables, response: output.response }, signal, host);
      output = { ...result, visualization: result.visualization ?? output.visualization, logs: [...output.logs, ...result.logs], tests: [...output.tests, ...result.tests] };
    } catch (cause) {
      if (signal?.aborted) throw cause;
      const partial = cause instanceof OpenApiScriptError ? cause.output : undefined;
      throw new OpenApiScriptError(input.kind, index + 1, cause instanceof Error ? cause.message : "script_failed", { ...output, logs: [...output.logs, ...(partial?.logs ?? [])], tests: [...output.tests, ...(partial?.tests ?? [])] });
    }
  }
  return output;
}

export const OPENAPI_SCRIPT_MAX_LENGTH = 32 * 1024;
export const OPENAPI_SCRIPT_TIMEOUT_MS = 5_000;
const SENSITIVE_VARIABLE = /(?:authorization|cookie|password|passwd|secret|token|api[-_]?key|credential|signature|privatekey)/i;

export function scriptVariables(variables: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(variables).filter(([name]) => !SENSITIVE_VARIABLE.test(name)));
}

export function validateOpenApiScript(value: string): string | null {
  if (value.length > OPENAPI_SCRIPT_MAX_LENGTH) return "script_too_large";
  return null;
}

export function runOpenApiScript(input: OpenApiScriptInput, signal?: AbortSignal, host?: OpenApiScriptHost): Promise<OpenApiScriptOutput> {
  if (signal?.aborted) return Promise.reject(new DOMException("Aborted", "AbortError"));
  const validationError = validateOpenApiScript(input.script);
  if (validationError) return Promise.reject(new Error(validationError));
  const safeInput = {
    ...input,
    variables: scriptVariables(input.variables),
    localVariables: input.localVariables ? scriptVariables(input.localVariables) : undefined,
    scopes: input.scopes ? { environment: scriptVariables(input.scopes.environment), globals: scriptVariables(input.scopes.globals) } : undefined,
  };
  if (!input.script.trim()) return Promise.resolve({ request: input.request, variables: { ...safeInput.variables }, scopes: safeInput.scopes, localVariables: safeInput.localVariables, response: input.response, logs: [], tests: [] });

  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./openapiScriptWorker.ts", import.meta.url), { type: "module" });
    let timeout: ReturnType<typeof setTimeout>;
    const pending = new AbortController();
    const cleanup = () => {
      worker.terminate();
      globalThis.clearTimeout(timeout);
      signal?.removeEventListener("abort", abort);
      pending.abort();
    };
    const fail = (error: Error) => {
      cleanup();
      reject(error);
    };
    const armTimeout = (ms: number) => {
      globalThis.clearTimeout(timeout);
      timeout = globalThis.setTimeout(() => fail(new Error("script_timeout")), ms);
    };
    const abort = () => fail(new DOMException("Aborted", "AbortError"));
    // Loading the WASM asset is separate from the script's execution budget.
    armTimeout(20_000);
    signal?.addEventListener("abort", abort, { once: true });
    worker.addEventListener("message", async (event: MessageEvent<OpenApiScriptOutput | { ready: true } | { error: string; output?: OpenApiScriptOutput } | { rpc: number; request: OpenApiScriptHttpRequest }>) => {
      if ("ready" in event.data) {
        armTimeout(OPENAPI_SCRIPT_TIMEOUT_MS + 250);
        return;
      }
      if ("rpc" in event.data) {
        const { rpc, request } = event.data;
        try {
          if (!host?.sendRequest) throw new Error("script_http_unavailable");
          const response = await host.sendRequest(request, pending.signal);
          if (!pending.signal.aborted) worker.postMessage({ rpc, response });
        } catch (error) {
          if (!pending.signal.aborted) worker.postMessage({ rpc, error: error instanceof Error ? error.message : "script_http_failed" });
        }
        return;
      }
      cleanup();
      if ("error" in event.data) reject(new OpenApiScriptError(input.kind, 1, event.data.error, event.data.output));
      else resolve(event.data);
    });
    worker.addEventListener(
      "error",
      (event) => {
        fail(new Error(event.message || "script_worker_failed"));
      },
      { once: true },
    );
    worker.postMessage(JSON.parse(JSON.stringify(safeInput)));
  });
}
