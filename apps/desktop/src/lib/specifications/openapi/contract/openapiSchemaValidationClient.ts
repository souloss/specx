import type { OpenApiSchemaValidationInput, OpenApiSchemaValidationResult } from "./openapiSchemaValidation";

export function validateOpenApiSchemaInWorker(input: OpenApiSchemaValidationInput, signal?: AbortSignal): Promise<OpenApiSchemaValidationResult> {
  if (signal?.aborted) return Promise.reject(new DOMException("Aborted", "AbortError"));
  return new Promise((resolve, reject) => {
    let worker: Worker;
    try {
      worker = new Worker(new URL("./openapiSchemaValidationWorker.ts", import.meta.url), { type: "module" });
    } catch {
      resolve({ valid: false, issues: [], failure: "worker" });
      return;
    }
    const finish = (result: OpenApiSchemaValidationResult) => {
      worker.terminate();
      globalThis.clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      resolve(result);
    };
    const abort = () => {
      worker.terminate();
      globalThis.clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      reject(new DOMException("Aborted", "AbortError"));
    };
    const timer = globalThis.setTimeout(() => finish({ valid: false, issues: [], failure: "timeout" }), 2_000);
    signal?.addEventListener("abort", abort, { once: true });
    worker.addEventListener("message", (event: MessageEvent<OpenApiSchemaValidationResult>) => finish(event.data), { once: true });
    worker.addEventListener("error", () => finish({ valid: false, issues: [], failure: "worker" }), { once: true });
    try {
      worker.postMessage(JSON.parse(JSON.stringify(input)));
    } catch {
      finish({ valid: false, issues: [], failure: "worker" });
    }
  });
}
