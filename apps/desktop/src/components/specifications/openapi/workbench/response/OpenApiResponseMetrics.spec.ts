import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiResponseMetrics from "./OpenApiResponseMetrics.vue";
import type { OpenApiResponseTelemetry } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";

const metrics: OpenApiResponseTelemetry = {
  transport: "agent",
  totalTime: 10,
  steps: [],
  detailed: true,
  responseSize: { total: 2, headers: 0, body: 2, estimated: false, partial: false },
  requestSize: { total: 0, headers: 0, body: 0, estimated: false, partial: false },
  decodedBodySize: 2,
  capturedBodySize: 2,
  url: "https://example.test",
  protocol: "HTTP/2",
  encrypted: true,
  redirected: false,
  cached: false,
  network: { localIp: "127.0.0.1", localPort: 1234, remoteIp: "1.1.1.1", remotePort: 443, protocol: "HTTP/2", tls: { protocol: "TLSv1.3", cipher: "TLS_AES_256_GCM_SHA384", certificateCommonName: "example.test", issuerCommonName: "Test CA", expiresAt: "Nov 16 12:00:00 2027 GMT" } },
};
function render(input = metrics) {
  return mount(OpenApiResponseMetrics, { props: { status: 200, statusText: "OK", metrics: input }, global: { stubs: { OpenApiResponseMetric: { template: '<section><slot /><slot name="details" /></section>' }, AppTooltip: { template: "<span><slot /></span>" } } } });
}
describe("response connection details", () => {
  it("renders the negotiated TLS and certificate fields returned by Agent", () => {
    const wrapper = render();
    for (const value of ["TLSv1.3", "TLS_AES_256_GCM_SHA384", "example.test", "Test CA", "Nov 16 12:00:00 2027 GMT"]) expect(wrapper.text()).toContain(value);
    expect(wrapper.text()).not.toContain("不可用");
    wrapper.unmount();
  });
  it("marks plaintext TLS details as not applicable", () => {
    const wrapper = render({ ...metrics, encrypted: false, url: "http://example.test", network: { ...metrics.network!, tls: null } });
    expect(wrapper.text()).toContain("不适用");
    expect(wrapper.text()).not.toContain("TLSv1.3");
    wrapper.unmount();
  });
});
