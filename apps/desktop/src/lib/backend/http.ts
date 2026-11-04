import { ApiError, type BackendTransport, type RequestOptions } from "./types";

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers = new Headers(options.headers);
  let body: BodyInit | undefined;
  if (options.body !== undefined) {
    if (typeof options.body === "string") body = options.body;
    else {
      headers.set("content-type", "application/json");
      body = JSON.stringify(options.body);
    }
  }
  const response = await fetch(path, {
    method: options.method ?? "GET",
    headers,
    body,
    credentials: "include",
  });
  const responseBody = response.status === 204 ? undefined : await response.json().catch(() => undefined);
  if (!response.ok) throw ApiError.fromResponse(responseBody, response.status);
  return responseBody as T;
}

export const httpTransport: BackendTransport = { request };
