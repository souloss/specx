import { describe, expect, it, vi } from "vitest";
import { buildResponseTelemetry, findResponseResourceTiming, observeResponseResourceTiming, formatResponseBytes, formatResponseDuration, requestBodyByteSize, type ResponseResourceTiming } from "./openapiResponseTelemetry";

const resource: ResponseResourceTiming = {
  name: "https://example.test/items",
  initiatorType: "fetch",
  startTime: 20,
  fetchStart: 21,
  domainLookupStart: 22,
  domainLookupEnd: 24,
  connectStart: 24,
  secureConnectionStart: 30,
  connectEnd: 40,
  requestStart: 41,
  responseStart: 100,
  responseEnd: 120,
  transferSize: 380,
  encodedBodySize: 80,
  decodedBodySize: 160,
  nextHopProtocol: "h2",
};
const input = {
  preparedAt: 0,
  startedAt: 20,
  headersAt: 101,
  receivedAt: 121,
  completedAt: 125,
  request: { method: "POST", url: resource.name, headers: [["content-type", "text/plain"]] as Array<[string, string]>, bodyBytes: 3 },
  response: { status: 200, statusText: "OK", url: resource.name, headers: [["content-type", "text/plain"]] as Array<[string, string]>, receivedBytes: 160, truncated: false, redirected: false },
};

describe("response telemetry", () => {
  it("separates TCP and TLS, includes preparation and processing, and marks socket data unavailable", () => {
    const result = buildResponseTelemetry({ ...input, resource });
    expect(result.totalTime).toBe(125);
    expect(Object.fromEntries(result.steps.map((step) => [step.phase, step.duration]))).toEqual({ prepare: 20, socket: null, dns: 2, tcp: 6, tls: 10, ttfb: 59, download: 20, process: 4 });
    expect(result.responseSize).toEqual({ total: 380, headers: 300, body: 80, estimated: true, partial: false });
    expect(result.decodedBodySize).toBe(160);
    expect(result.protocol).toBe("h2");
  });

  it("never turns CORS-redacted timing into zero-duration phases or exact transfer sizes", () => {
    const result = buildResponseTelemetry({ ...input, resource: { ...resource, requestStart: 0, responseStart: 0, transferSize: 0, encodedBodySize: 0, decodedBodySize: 0, nextHopProtocol: "" } });
    expect(result.detailed).toBe(false);
    expect(result.steps.find((s) => s.phase === "dns")?.duration).toBeNull();
    expect(result.steps.find((s) => s.phase === "ttfb")?.duration).toBeNull();
    expect(result.steps.find((s) => s.phase === "waiting")?.duration).toBe(81);
    expect(result.responseSize.body).toBe(160);
    expect(result.responseSize.estimated).toBe(true);
    expect(result.protocol).toBeNull();
  });

  it("marks truncated bodies and unknown multipart serialization as partial measurements", () => {
    const result = buildResponseTelemetry({ ...input, request: { ...input.request, bodyBytes: null }, response: { ...input.response, truncated: true, receivedBytes: 64 }, resource });
    expect(result.detailed).toBe(false);
    expect(result.responseSize).toMatchObject({ partial: true, body: 64 });
    expect(result.requestSize).toMatchObject({ partial: true, body: null });
    expect(result.capturedBodySize).toBe(64);
  });

  it("distinguishes a cached resource from a zero-length response", () => {
    const cached = buildResponseTelemetry({ ...input, resource: { ...resource, transferSize: 0 } });
    expect(cached.cached).toBe(true);
    expect(cached.responseSize.body).toBe(80);
    const empty = buildResponseTelemetry({ ...input, response: { ...input.response, status: 204, receivedBytes: 0 }, resource: { ...resource, transferSize: 300, encodedBodySize: 0, decodedBodySize: 0 } });
    expect(empty.cached).toBe(false);
    expect(empty.responseSize.body).toBe(0);
  });

  it("does not reuse stale entries, XHRs or unfinished requests sharing a URL", () => {
    const stale = { ...resource, startTime: 0 };
    const unfinished = { ...resource, responseEnd: 0 };
    const xhr = { ...resource, initiatorType: "xmlhttprequest" };
    expect(findResponseResourceTiming([stale, unfinished, xhr, resource], [resource.name], 20, 121)).toBe(resource);
    expect(findResponseResourceTiming([stale, unfinished], [resource.name], 20, 121)).toBeUndefined();
  });

  it("counts UTF-8 bytes and serialized form data without inventing multipart boundaries", () => {
    expect(requestBodyByteSize("猫")).toBe(3);
    expect(requestBodyByteSize(new URLSearchParams({ q: "a b" }))).toBe(5);
    expect(requestBodyByteSize(new Blob(["abc"]))).toBe(3);
    expect(requestBodyByteSize(new Uint8Array([1, 2]))).toBe(2);
    expect(requestBodyByteSize(undefined)).toBe(0);
    expect(requestBodyByteSize(new FormData())).toBeNull();
  });

  it("captures entries through its observer when the shared performance buffer is full, then disconnects", async () => {
    const observer = { observe: vi.fn(), takeRecords: () => [resource], disconnect: vi.fn() };
    vi.stubGlobal(
      "PerformanceObserver",
      vi.fn(function () {
        return observer;
      }),
    );
    vi.stubGlobal("performance", { getEntriesByType: () => [] });
    try {
      const capture = observeResponseResourceTiming(resource.name, 20);
      expect(await capture.finish(121, resource.name)).toBe(resource);
      expect(observer.disconnect).toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("formats compact summaries and precise timing details separately", () => {
    expect(formatResponseDuration(3051.3)).toBe("3.05 s");
    expect(formatResponseDuration(3051.3, true)).toBe("3051.30 ms");
    expect(formatResponseBytes(528)).toBe("528 B");
    expect(formatResponseBytes(1024)).toBe("1.00 KB");
  });
});
