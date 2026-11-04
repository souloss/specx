import { isTauriRuntime } from "./runtime";
import type { BackendTransport } from "./types";

let transportPromise: Promise<BackendTransport> | undefined;

export function getBackendTransport(): Promise<BackendTransport> {
  transportPromise ??= isTauriRuntime() ? import("./tauri").then(({ desktopTransport }) => desktopTransport) : import("./http").then(({ httpTransport }) => httpTransport);
  return transportPromise;
}
