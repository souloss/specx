import { newQuickJSWASMModuleFromVariant, newVariant } from "quickjs-emscripten-core";
import RELEASE_SYNC from "@jitl/quickjs-wasmfile-release-sync";
import quickjsWasmUrl from "../../../../../../node_modules/@jitl/quickjs-wasmfile-release-sync/dist/emscripten-module.wasm?url";
import { executeOpenApiScript } from "./openapiScriptRuntime";
import { OpenApiScriptError } from "./openapiScriptSandbox";
import type { OpenApiScriptInput, OpenApiScriptHttpRequest } from "./openapiScriptSandbox";
import type { WorkbenchResponse } from "../openapiWorkbench";

let nextRpc = 0;
const pending = new Map<number, { resolve: (response: WorkbenchResponse) => void; reject: (error: Error) => void }>();
self.addEventListener("message", async (event: MessageEvent<OpenApiScriptInput | { rpc: number; response?: WorkbenchResponse; error?: string }>) => {
  if ("rpc" in event.data) {
    const reply = event.data,
      task = pending.get(reply.rpc);
    pending.delete(reply.rpc);
    if (reply.response) task?.resolve(reply.response);
    else task?.reject(new Error(reply.error ?? "script_http_failed"));
    return;
  }
  try {
    const QuickJS = await newQuickJSWASMModuleFromVariant(newVariant(RELEASE_SYNC, { wasmLocation: quickjsWasmUrl }));
    self.postMessage({ ready: true });
    const result = await executeOpenApiScript(QuickJS, event.data, {
      sendRequest: (request: OpenApiScriptHttpRequest) =>
        new Promise((resolve, reject) => {
          const rpc = ++nextRpc;
          pending.set(rpc, { resolve, reject });
          self.postMessage({ rpc, request });
        }),
    });
    self.postMessage(result);
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : "script_failed", output: error instanceof OpenApiScriptError ? error.output : undefined });
  }
});
