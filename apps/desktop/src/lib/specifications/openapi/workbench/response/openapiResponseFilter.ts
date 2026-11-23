export type ResponseFilterResult = { count: number; text: string } | { error: "syntax" | "evaluation" | "limit" | "timeout" | "unavailable"; detail?: string };

export function filterOpenApiResponse(body: string, query: string, signal: AbortSignal): Promise<ResponseFilterResult> {
  const unavailable = { error: "unavailable" } as const;
  if (signal.aborted) return Promise.resolve(unavailable);
  return new Promise((resolve) => {
    let worker: Worker;
    try {
      worker = new Worker(new URL("./openapiResponseFilterWorker.ts", import.meta.url), { type: "module" });
    } catch {
      resolve(unavailable);
      return;
    }
    function finish(result: ResponseFilterResult) {
      worker.terminate();
      globalThis.clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      resolve(result);
    }
    function abort() {
      finish(unavailable);
    }
    const timer = globalThis.setTimeout(() => finish({ error: "timeout" }), 3_000);
    signal.addEventListener("abort", abort, { once: true });
    worker.onmessage = (event: MessageEvent<ResponseFilterResult>) => finish(event.data);
    worker.onerror = () => finish(unavailable);
    try {
      worker.postMessage({ body, query });
    } catch {
      finish(unavailable);
    }
  });
}
