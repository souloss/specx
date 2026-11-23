import type { AgentResponse } from "@/types/api";

export type ResponseTimingPhase = "prepare" | "socket" | "dns" | "tcp" | "tls" | "ttfb" | "waiting" | "download" | "process";
export type ResponseTimingStep = { phase: ResponseTimingPhase; duration: number | null };
export type ResponseSizeMeasurement = { total: number; headers: number; body: number | null; estimated: boolean; partial: boolean };
export type OpenApiResponseTelemetry = {
  transport?: "browser" | "agent";
  network?: AgentResponse["network"];
  totalTime: number;
  steps: ResponseTimingStep[];
  detailed: boolean;
  responseSize: ResponseSizeMeasurement;
  requestSize: ResponseSizeMeasurement;
  decodedBodySize: number;
  capturedBodySize: number;
  url: string;
  protocol: string | null;
  encrypted: boolean;
  redirected: boolean;
  cached: boolean;
};

export type ResponseResourceTiming = Pick<
  PerformanceResourceTiming,
  "name" | "initiatorType" | "startTime" | "fetchStart" | "domainLookupStart" | "domainLookupEnd" | "connectStart" | "connectEnd" | "secureConnectionStart" | "requestStart" | "responseStart" | "responseEnd" | "transferSize" | "encodedBodySize" | "decodedBodySize" | "nextHopProtocol"
>;

/** A previous request to the same URL must never supply this request's metrics. */
export function findResponseResourceTiming(entries: ResponseResourceTiming[], urls: string[], startedAt: number, receivedAt: number): ResponseResourceTiming | undefined {
  const names = urls.map((url) => url.split("#", 1)[0]);
  return entries.filter((entry) => entry.initiatorType === "fetch" && names.includes(entry.name) && entry.startTime >= startedAt && entry.responseEnd > 0 && entry.responseEnd <= receivedAt + 1).sort((a, b) => a.startTime - b.startTime)[0];
}

/** Capture this request even when the page's shared Resource Timing buffer is full. */
export function observeResponseResourceTiming(url: string, startedAt: number) {
  const entries: ResponseResourceTiming[] = [];
  let observer: PerformanceObserver | undefined;
  const name = new URL(url, typeof location === "undefined" ? "http://localhost" : location.href).href.split("#", 1)[0];
  const collect = (items: PerformanceEntry[]) => {
    entries.push(...(items.filter((item) => item.name === name) as PerformanceResourceTiming[]));
  };
  if (typeof PerformanceObserver !== "undefined") {
    try {
      observer = new PerformanceObserver((list) => collect(list.getEntries()));
      observer.observe({ entryTypes: ["resource"] });
    } catch {
      observer?.disconnect();
      observer = undefined;
    }
  }
  return {
    async finish(receivedAt: number, finalUrl: string) {
      // A fetch body can finish before delivery of the observer notification task.
      // Yield once without clearing or enlarging the page-wide performance buffer.
      if (observer) await new Promise<void>((resolve) => setTimeout(resolve, 0));
      collect(observer?.takeRecords() ?? []);
      const buffered = (performance.getEntriesByType?.("resource") ?? []) as PerformanceResourceTiming[];
      observer?.disconnect();
      return findResponseResourceTiming([...entries, ...buffered], [name, finalUrl], startedAt, receivedAt);
    },
    disconnect() {
      observer?.disconnect();
    },
  };
}

export function requestBodyByteSize(body: BodyInit | undefined): number | null {
  if (body === undefined || body === null) return 0;
  if (typeof body === "string" || body instanceof URLSearchParams) return new TextEncoder().encode(String(body)).byteLength;
  if (body instanceof Blob) return body.size;
  if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) return body.byteLength;
  // Fetch owns multipart boundaries and stream serialization; do not report a guessed size.
  return null;
}

function headerByteSize(firstLine: string, headers: Array<[string, string]>): number {
  return new TextEncoder().encode(`${firstLine}\r\n${headers.map(([name, value]) => `${name}: ${value}\r\n`).join("")}\r\n`).byteLength;
}

export function buildResponseTelemetry(input: {
  preparedAt: number;
  startedAt: number;
  headersAt: number;
  receivedAt: number;
  completedAt: number;
  request: { method: string; url: string; headers: Array<[string, string]>; bodyBytes: number | null };
  response: { status: number; statusText: string; url: string; headers: Array<[string, string]>; receivedBytes: number; truncated: boolean; redirected: boolean };
  resource?: ResponseResourceTiming;
}): OpenApiResponseTelemetry {
  const { request, response, resource } = input;
  const elapsed = (start: number, end: number) => Math.max(0, end - start);
  const detailed = Boolean(resource && resource.requestStart > 0 && resource.responseStart >= resource.requestStart && !response.truncated);
  const url = new URL(response.url || request.url, "http://localhost");
  const requestUrl = new URL(request.url, url);
  const requestHeaders = headerByteSize(`${request.method} ${requestUrl.pathname}${requestUrl.search} HTTP/1.1`, request.headers);
  const responseHeaders = headerByteSize(`HTTP/1.1 ${response.status} ${response.statusText}`, response.headers);
  // Resource Timing's transfer size includes a standardized header estimate, not wire-accurate headers.
  const transferAvailable = detailed && resource!.transferSize > 0 && resource!.transferSize >= resource!.encodedBodySize;
  const bodyBytes = detailed ? resource!.encodedBodySize : response.receivedBytes;
  const steps: ResponseTimingStep[] = [
    { phase: "prepare", duration: elapsed(input.preparedAt, input.startedAt) },
    { phase: "socket", duration: null },
    { phase: "dns", duration: detailed ? elapsed(resource!.domainLookupStart, resource!.domainLookupEnd) : null },
    { phase: "tcp", duration: detailed ? elapsed(resource!.connectStart, resource!.secureConnectionStart > 0 ? resource!.secureConnectionStart : resource!.connectEnd) : null },
    { phase: "tls", duration: detailed ? (resource!.secureConnectionStart > 0 ? elapsed(resource!.secureConnectionStart, resource!.connectEnd) : 0) : null },
    { phase: "ttfb", duration: detailed ? elapsed(resource!.requestStart, resource!.responseStart) : null },
    ...(!detailed ? [{ phase: "waiting" as const, duration: elapsed(input.startedAt, input.headersAt) }] : []),
    { phase: "download", duration: detailed ? elapsed(resource!.responseStart, resource!.responseEnd) : elapsed(input.headersAt, input.receivedAt) },
    { phase: "process", duration: elapsed(input.receivedAt, input.completedAt) },
  ];
  return {
    totalTime: elapsed(input.preparedAt, input.completedAt),
    steps,
    detailed,
    responseSize: {
      total: transferAvailable ? resource!.transferSize : responseHeaders + bodyBytes,
      headers: transferAvailable ? resource!.transferSize - resource!.encodedBodySize : responseHeaders,
      body: bodyBytes,
      estimated: true,
      partial: response.truncated,
    },
    requestSize: { total: requestHeaders + (request.bodyBytes ?? 0), headers: requestHeaders, body: request.bodyBytes, estimated: true, partial: request.bodyBytes === null },
    decodedBodySize: detailed ? resource!.decodedBodySize : response.receivedBytes,
    capturedBodySize: response.receivedBytes,
    url: url.href,
    protocol: resource?.nextHopProtocol || null,
    encrypted: url.protocol === "https:",
    redirected: response.redirected,
    cached: Boolean(detailed && resource!.transferSize === 0 && resource!.encodedBodySize > 0),
  };
}

export function formatResponseDuration(ms: number, detailed = false): string {
  if (detailed) return `${ms.toFixed(2)} ms`;
  return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.round(ms)} ms`;
}

export function formatResponseBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/** Agent values come from the target connection, independent of the API round trip. */
export function buildAgentResponseTelemetry(agent: AgentResponse, capturedBytes: number, truncated: boolean): OpenApiResponseTelemetry {
  const { timing, network } = agent;
  return {
    totalTime: timing.totalMs,
    steps: [
      { phase: "dns", duration: timing.dnsMs },
      { phase: "tcp", duration: timing.connectMs },
      { phase: "tls", duration: timing.tlsMs },
      { phase: "ttfb", duration: timing.firstByteMs },
      { phase: "download", duration: timing.downloadMs },
    ],
    detailed: true,
    transport: "agent",
    network,
    responseSize: { total: agent.responseHeaderBytes + agent.responseBodyBytes, headers: agent.responseHeaderBytes, body: agent.responseBodyBytes, estimated: false, partial: truncated },
    requestSize: { total: agent.requestHeaderBytes + agent.requestBodyBytes, headers: agent.requestHeaderBytes, body: agent.requestBodyBytes, estimated: false, partial: false },
    decodedBodySize: capturedBytes,
    capturedBodySize: capturedBytes,
    url: agent.url,
    protocol: network.protocol,
    encrypted: agent.url.startsWith("https:"),
    redirected: agent.redirects > 0,
    cached: false,
  };
}
