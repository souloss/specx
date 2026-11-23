import { executeAgentRequest, serializeAgentRequest, base64ToBytes, type RequestProxyMode } from "../request/openapiRequestAgent";
import type { OpenApiScriptHost } from "./openapiScriptSandbox";

/** Script subrequests use the selected request transport and never inherit app credentials. */
export function createOpenApiScriptHttpHost(options: { proxy: Exclude<RequestProxyMode, "auto">; workspaceId?: string; serviceId?: string; baseUrl: string; timeoutMs: number; followRedirects: boolean }): OpenApiScriptHost {
  return {
    async sendRequest(input, signal) {
      const url = new URL(input.url, options.baseUrl || globalThis.location?.href);
      if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) throw new Error("script_http_invalid_url");
      const headers = new Headers(input.headers);
      if (input.mediaType && !headers.has("content-type")) headers.set("content-type", input.mediaType);
      if (options.proxy === "agent") {
        if (!options.workspaceId || !options.serviceId) throw new Error("script_http_agent_unavailable");
        const request = await serializeAgentRequest({ url: url.href, method: input.method, headers, body: input.body, timeoutMs: options.timeoutMs, followRedirects: options.followRedirects });
        const response = await executeAgentRequest(options.workspaceId, options.serviceId, request, signal ?? new AbortController().signal);
        return { status: response.status, statusText: response.statusText, duration: response.timing.totalMs, headers: response.headers, body: new TextDecoder().decode(base64ToBytes(response.bodyBase64)), truncated: response.truncated };
      }
      const started = performance.now();
      const response = await fetch(url, { method: input.method, headers, body: input.body, signal, credentials: "omit", redirect: options.followRedirects ? "follow" : "error" });
      const reader = response.body?.getReader();
      const chunks: Uint8Array[] = [];
      let bytes = 0,
        truncated = false;
      const limit = 2 * 1024 * 1024;
      if (reader)
        try {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            const remaining = limit - bytes;
            chunks.push(value.subarray(0, remaining));
            bytes += Math.min(value.byteLength, remaining);
            if (value.byteLength > remaining) {
              truncated = true;
              await reader.cancel();
              break;
            }
          }
        } finally {
          reader.releaseLock();
        }
      const body = new Uint8Array(bytes);
      let offset = 0;
      for (const chunk of chunks) {
        body.set(chunk, offset);
        offset += chunk.length;
      }
      return { status: response.status, statusText: response.statusText, duration: Math.round(performance.now() - started), headers: [...response.headers], body: new TextDecoder().decode(body), truncated };
    },
  };
}
