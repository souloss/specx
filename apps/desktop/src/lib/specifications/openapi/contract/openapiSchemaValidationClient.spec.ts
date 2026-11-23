import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { validateOpenApiSchemaInWorker } from "./openapiSchemaValidationClient";

class ValidationWorker extends EventTarget {
  static current: ValidationWorker;
  terminate = vi.fn();
  postMessage = vi.fn();
  constructor() {
    super();
    ValidationWorker.current = this;
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("Worker", ValidationWorker);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
const input = { document: { openapi: "3.1.0" }, schema: { type: "string" }, body: "value" };

describe("contract validation Worker lifecycle", () => {
  it("terminates stalled validation and reports timeout instead of success", async () => {
    const result = validateOpenApiSchemaInWorker(input);
    await vi.advanceTimersByTimeAsync(2_000);
    expect(await result).toEqual({ valid: false, issues: [], failure: "timeout" });
    expect(ValidationWorker.current.terminate).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("terminates and releases its timer when the operation is cancelled", async () => {
    const controller = new AbortController();
    const result = validateOpenApiSchemaInWorker(input, controller.signal);
    const rejected = expect(result).rejects.toMatchObject({ name: "AbortError" });
    controller.abort();
    await rejected;
    expect(ValidationWorker.current.terminate).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("cleans up successful results and reports unavailable Workers", async () => {
    const result = validateOpenApiSchemaInWorker(input);
    ValidationWorker.current.dispatchEvent(new MessageEvent("message", { data: { valid: true, issues: [] } }));
    expect(await result).toEqual({ valid: true, issues: [] });
    expect(ValidationWorker.current.terminate).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
    vi.stubGlobal(
      "Worker",
      class {
        constructor() {
          throw new Error("Private host details");
        }
      },
    );
    expect(await validateOpenApiSchemaInWorker(input)).toEqual({ valid: false, issues: [], failure: "worker" });
  });
});
