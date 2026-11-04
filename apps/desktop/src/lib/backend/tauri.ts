import { invoke } from "@tauri-apps/api/core";
import type { DesktopApiRequest, DesktopApiResponse } from "../../types/api";
import { ApiError, type BackendTransport, type RequestOptions } from "./types";

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const request: DesktopApiRequest = {
    method: options.method ?? "GET",
    path,
    headers: options.headers ?? {},
    body: options.body === undefined ? undefined : typeof options.body === "string" ? options.body : JSON.stringify(options.body),
  };
  if (request.body !== undefined) request.headers["content-type"] ??= "application/json";
  const response = await invoke<DesktopApiResponse>("specx_api_request", { request });
  if (response.status < 200 || response.status >= 300) {
    throw ApiError.fromResponse(response.body, response.status);
  }
  return response.body as T;
}

export const desktopTransport: BackendTransport = { request };
