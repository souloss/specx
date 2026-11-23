import { specxApi } from "@/lib/backend/client";
import { uuid } from "@/lib/common/utils";
import type { AgentRequest, AgentResponse, OpenApiEnvironmentServer } from "@/types/api";

export type RequestProxyMode = "auto" | "browser" | "agent";
export type EnvironmentProxyMode = OpenApiEnvironmentServer["requestMode"];
export function selectedRequestProxy(user: RequestProxyMode, environment?: EnvironmentProxyMode): "browser" | "agent" {
  return environment && environment !== "inherit" ? environment : user === "browser" ? "browser" : "agent";
}
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return btoa(binary);
}
export function base64ToBytes(text: string): Uint8Array {
  return Uint8Array.from(atob(text), (character) => character.charCodeAt(0));
}
/** A platform Request supplies the exact multipart boundary and binary upload bytes. */
export async function serializeAgentRequest(input: { method: string; url: string; headers: Headers; disabledHeaders?: string[]; body?: BodyInit; timeoutMs: number; followRedirects: boolean }): Promise<AgentRequest> {
  // Request's browser guard strips Cookie and other forbidden headers. Use it
  // only for body serialization, then send our explicit header list to Agent.
  const headers = new Headers(input.headers);
  const request = new Request(input.url, { method: input.method, body: input.body });
  const bytes = new Uint8Array(await request.arrayBuffer());
  const contentType = request.headers.get("content-type");
  if (!headers.has("content-type") && contentType) headers.set("content-type", contentType);
  const disabledHeaders = [...new Set((input.disabledHeaders ?? []).map((name) => name.toLowerCase()))];
  for (const name of disabledHeaders) headers.delete(name);
  return { method: input.method, url: input.url, ...(disabledHeaders.length ? { disabledHeaders } : {}), headers: [...headers.entries()], bodyBase64: bytesToBase64(bytes), timeoutMs: input.timeoutMs, followRedirects: input.followRedirects };
}
export async function executeAgentRequest(workspaceId: string, serviceId: string, request: AgentRequest, signal: AbortSignal): Promise<AgentResponse> {
  signal.throwIfAborted();
  const id = uuid();
  const cancel = () => {
    void specxApi.cancelRequestAgent(workspaceId, serviceId, id).catch(() => {});
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const result = await specxApi.executeRequestAgent(workspaceId, serviceId, id, request, signal);
    signal.throwIfAborted();
    return result;
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}
