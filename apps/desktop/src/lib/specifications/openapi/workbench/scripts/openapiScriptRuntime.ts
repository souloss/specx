import { createOpenApiVariableResolver } from "../variables/openapiDynamicVariables";
import chaiSource from "chai/index.js?raw";
import Ajv from "ajv";
import addFormats from "ajv-formats";
import { renderResponseVisualization } from "../response/openapiResponseVisualization";
import { OPENAPI_SCRIPT_API } from "./openapiScriptApi";
import { OPENAPI_SCRIPT_TIMEOUT_MS, OpenApiScriptError, scriptVariables } from "./openapiScriptSandbox";
import type { QuickJSContext, QuickJSHandle, QuickJSWASMModule } from "quickjs-emscripten-core";
import type { OpenApiScriptHost, OpenApiScriptInput, OpenApiScriptOutput, OpenApiScriptHttpRequest } from "./openapiScriptSandbox";

const MAX_LOGS = 500;
const MAX_OUTPUT_BYTES = 1024 * 1024;
const ajv = new Ajv({ strict: false, allErrors: true });
addFormats(ajv);

/** Runs the actual engine in a separate realm. Only explicit JSON host bridges cross it. */
export async function executeOpenApiScript(QuickJS: QuickJSWASMModule, input: OpenApiScriptInput, host: OpenApiScriptHost = {}, signal?: AbortSignal): Promise<OpenApiScriptOutput> {
  const state: OpenApiScriptOutput = { request: structuredClone(input.request), variables: scriptVariables(input.variables), logs: [], tests: [] };
  const runtime = QuickJS.newRuntime();
  runtime.setMemoryLimit(32 * 1024 * 1024);
  runtime.setMaxStackSize(512 * 1024);
  const deadline = Date.now() + OPENAPI_SCRIPT_TIMEOUT_MS;
  runtime.setInterruptHandler(() => Date.now() > deadline || Boolean(signal?.aborted));
  runtime.setModuleLoader((name) => {
    if (name !== "chai") throw new Error("script_module_unavailable: " + name);
    return chaiSource;
  });
  const vm = runtime.newContext();
  const pending = new Set<ReturnType<QuickJSContext["newPromise"]>>();
  const timers = new Map<number, () => void>();
  let active = true;
  const str = (handle: QuickJSHandle) => vm.getString(handle);
  const evaluate = (source: string, module = false) => {
    const result = vm.evalCode(source, module ? "specx-chai.mjs" : "specx-script.js", { type: module ? "module" : "global" });
    if (result.error) {
      const error = vm.dump(result.error);
      result.error.dispose();
      throw new Error(error?.message ?? String(error));
    }
    return result.value;
  };
  const bridge = (name: string, fn: (...args: QuickJSHandle[]) => QuickJSHandle | void) => {
    const handle = vm.newFunction(name, (...args) => fn(...args) ?? vm.undefined);
    vm.setProp(vm.global, `__specx_${name}`, handle);
    handle.dispose();
  };
  try {
    const variableUpdates: Record<string, string> = {};
    const variableResolver = createOpenApiVariableResolver({
      locale: input.locale,
      onVariableUpdate: (name, value) => {
        variableUpdates[name] = value;
      },
    });
    bridge("expandVariables", (value, variables) => vm.newString(JSON.stringify({ value: variableResolver.expand(str(value), JSON.parse(str(variables))), updates: variableUpdates })));
    bridge("input", () => vm.newString(JSON.stringify({ ...input, variables: state.variables })));
    bridge("log", (value) => {
      if (state.logs.length < MAX_LOGS) state.logs.push(str(value).slice(0, 4000));
    });
    bridge("test", (value) => {
      if (state.tests.length >= MAX_LOGS) throw new Error("script_tests_limit");
      state.tests.push(JSON.parse(str(value)));
    });
    bridge("urlParts", (value) => {
      const raw = str(value);
      const hashIndex = raw.indexOf("#");
      const hash = hashIndex < 0 ? "" : raw.slice(hashIndex);
      const plain = hashIndex < 0 ? raw : raw.slice(0, hashIndex);
      const queryIndex = plain.indexOf("?");
      let parsed: URL | undefined;
      try {
        parsed = new URL(raw);
      } catch {
        /* Unexpanded variables remain available to scripts. */
      }
      return vm.newString(
        JSON.stringify({
          base: queryIndex < 0 ? plain : plain.slice(0, queryIndex),
          hash,
          query: [...new URLSearchParams(queryIndex < 0 ? "" : plain.slice(queryIndex + 1))].map(([key, value]) => ({ key, value })),
          hostname: parsed?.hostname ?? "",
          pathname: parsed?.pathname ?? "",
          origin: parsed?.origin ?? "",
        }),
      );
    });
    bridge("visualize", (template, data) => {
      const source = str(template),
        context = str(data);
      if (source.length > 32768 || context.length > 512 * 1024) throw new Error("visualization_too_large");
      const rendered = renderResponseVisualization(source, JSON.parse(context));
      if (rendered.length > MAX_OUTPUT_BYTES) throw new Error("visualization_too_large");
      state.visualization = rendered;
    });
    bridge("validateSchema", (schema, body) => {
      const validate = ajv.compile(JSON.parse(str(schema)));
      validate(JSON.parse(str(body)));
      return vm.newString(JSON.stringify({ errors: (validate.errors ?? []).map((error) => `${error.instancePath} ${error.message}`) }));
    });
    bridge("commit", (value) => {
      const result = JSON.parse(str(value)) as Pick<OpenApiScriptOutput, "request" | "variables" | "localVariables" | "scopes" | "response">;
      Object.assign(state, result);
    });
    // These bridges capture primitives synchronously before their borrowed VM handles expire.
    bridge("cancelDelay", (id) => {
      timers.get(vm.getNumber(id))?.();
    });
    bridge("delay", (ms, idHandle) => {
      const duration = vm.getNumber(ms),
        id = vm.getNumber(idHandle);
      const deferred = vm.newPromise();
      pending.add(deferred);
      const complete = () => {
        timers.delete(id);
        if (active) {
          deferred.resolve();
          pending.delete(deferred);
          deferred.dispose();
        }
      };
      const timer = setTimeout(complete, duration);
      timers.set(id, () => {
        clearTimeout(timer);
        complete();
      });
      return deferred.handle;
    });
    bridge("sendRequest", (value) => {
      const request = JSON.parse(str(value)) as OpenApiScriptHttpRequest;
      const deferred = vm.newPromise();
      pending.add(deferred);
      const operation = host.sendRequest ? host.sendRequest(request, signal) : Promise.reject(new Error("script_http_unavailable"));
      operation
        .then(
          (response) => {
            if (!active) return;
            const handle = vm.newString(JSON.stringify(response));
            deferred.resolve(handle);
            handle.dispose();
          },
          (error) => {
            if (!active) return;
            const handle = vm.newError(error instanceof Error ? error.message : String(error));
            deferred.reject(handle);
            handle.dispose();
          },
        )
        .finally(() => {
          if (active) {
            pending.delete(deferred);
            deferred.dispose();
          }
        });
      return deferred.handle;
    });
    evaluate('import { expect } from "chai"; globalThis.__specx_expect = expect;', true).dispose();
    evaluate(`(() => { ${OPENAPI_SCRIPT_API} })();`).dispose();
    const promise = evaluate(`(async () => {\n${input.script}\n})().then(async () => { await __specx_drain(); __specx_finish(); });`);
    try {
      while (true) {
        if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
        if (Date.now() > deadline) throw new Error("script_timeout");
        const jobs = runtime.executePendingJobs();
        if (jobs.error) {
          const error = vm.dump(jobs.error);
          jobs.error.dispose();
          throw new Error(error?.message ?? String(error));
        }
        const status = vm.getPromiseState(promise);
        if (status.type === "fulfilled") {
          status.value.dispose();
          break;
        }
        if (status.type === "rejected") {
          const error = vm.dump(status.error);
          status.error.dispose();
          throw new Error(error?.message ?? String(error));
        }
        await new Promise((resolve) => setTimeout(resolve, 2));
      }
    } finally {
      promise.dispose();
    }
    return state;
  } catch (error) {
    if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
    throw new OpenApiScriptError(input.kind, 1, error instanceof Error ? error.message : "script_failed", state);
  } finally {
    for (const stop of timers.values()) stop();
    active = false;
    for (const deferred of pending) deferred.dispose();
    vm.dispose();
    runtime.dispose();
  }
}
