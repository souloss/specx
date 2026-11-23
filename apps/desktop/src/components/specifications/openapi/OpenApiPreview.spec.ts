import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { installMemoryStorage } from "@/test/browserStorage";
import OpenApiPreview from "./OpenApiPreview.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { bytesToBase64 } from "@/lib/specifications/openapi/workbench/request/openapiRequestAgent";
vi.mock("./shared/OpenApiCodeEditor.vue", () => ({ default: { template: '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />', props: ["modelValue"], emits: ["update:modelValue"] } }));

const content = `openapi: 3.0.3
info:
  title: Accounts API
  version: '1.0'
servers:
  - url: https://api.example.test
paths:
  /accounts/{accountId}:
    get:
      operationId: listAccounts
      summary: List accounts
      parameters:
        - name: accountId
          in: path
          required: true
          description: Account identifier
          schema:
            type: string
        - name: limit
          in: query
          description: Maximum number of accounts
          schema:
            type: integer
      responses:
        '200':
          description: Accounts returned
`;

beforeEach(() => {
  installMemoryStorage();
  setActivePinia(createPinia());
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("OpenAPI preview", () => {
  it("shows Agent's final uploaded JSON instead of an editor draft or response body", async () => {
    useWorkspaceStore().catalog = catalogFixture();
    const uploaded = '{"title":"actually sent"}';
    const execute = vi.spyOn(specxApi, "executeRequestAgent").mockResolvedValue({
      status: 200,
      statusText: "OK",
      headers: [["content-type", "application/json"]],
      bodyBase64: bytesToBase64(new TextEncoder().encode('{"title":"response"}')),
      truncated: false,
      url: "https://api.example.test/final",
      redirects: 1,
      timing: { dnsMs: 1, connectMs: 1, tlsMs: 1, firstByteMs: 1, downloadMs: 1, totalMs: 5 },
      network: { localIp: "127.0.0.1", localPort: 1, remoteIp: "1.1.1.1", remotePort: 443, protocol: "HTTP/2", tls: null },
      requestMethod: "PUT",
      requestHeaders: [["Content-Type", "application/json"]],
      requestBodyBase64: bytesToBase64(new TextEncoder().encode(uploaded)),
      requestBodyBytes: uploaded.length,
      requestHeaderBytes: 10,
      responseHeaderBytes: 10,
      responseBodyBytes: 20,
    });
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", serviceId: "svc-a" } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(execute).toHaveBeenCalled());
    await wrapper.get("#openapi-response-tab-request").trigger("click");
    const actual = wrapper.get(".executed-request-view").text();
    expect(actual).toContain("PUT");
    expect(actual).toContain("https://api.example.test/final");
    expect(actual).toContain("actually sent");
    expect(actual).not.toContain('"response"');
    expect(actual).not.toContain("此请求未发送 Body");
    wrapper.unmount();
  });
  it("sends hidden-header checkbox changes to the Agent after body serialization", async () => {
    useWorkspaceStore().catalog = catalogFixture();
    const execute = vi.spyOn(specxApi, "executeRequestAgent").mockRejectedValue(new Error("offline"));
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", serviceId: "svc-a" } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    expect(wrapper.text()).not.toContain("暂存");
    expect(wrapper.text()).not.toContain("保存为用例");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get("#openapi-debug-section-tab-headers").trigger("click");
    await wrapper.get('button[aria-label="查看隐藏请求头"]').trigger("click");
    const generated = wrapper.findAll(".debug-generated-parameter");
    expect(generated.length).toBeGreaterThan(2);
    const accept = generated.find((row) => row.get<HTMLInputElement>(".debug-parameter-name input").element.value === "Accept")!;
    await accept.get('input[type="checkbox"]').setValue(false);
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(execute).toHaveBeenCalled());
    expect(execute.mock.calls[0]?.[3].disabledHeaders).toContain("accept");
    wrapper.unmount();
  });

  it("serializes a referenced custom Params object into query fields", async () => {
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const source = content + "components:\n  schemas:\n    Filter:\n      type: object\n      properties:\n        active:\n          type: boolean\n";
    const wrapper = mount(OpenApiPreview, { props: { content: source, focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get(".debug-field-group .debug-parameter-add-row input").setValue("filter");
    await wrapper.get('[name="openapi-query-type-0"]').setValue("#/components/schemas/Filter");
    expect(wrapper.find(".debug-structured-detail-row").exists()).toBe(true);
    await wrapper.get('[name="openapi-query-value-0-value-active"]').setValue("true");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(String(fetch.mock.calls[0]?.[0])).toContain("active=true");
    expect(String(fetch.mock.calls[0]?.[0])).not.toContain("filter=%7B");
    wrapper.unmount();
  });

  it("keeps route placeholders visible and applies OpenAPI matrix serialization only when executing", async () => {
    const source = JSON.parse(
      JSON.stringify({
        openapi: "3.0.3",
        info: { title: "Matrix", version: "1" },
        servers: [{ url: "https://api.example.test" }],
        paths: { "/accounts/{accountId}": { get: { parameters: [{ name: "accountId", in: "path", required: true, style: "matrix", explode: true, schema: { type: "array", items: { type: "string" } } }], responses: { "200": { description: "OK" } } } } },
      }),
    );
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response("{}", { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetch);
    const wrapper = mount(OpenApiPreview, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get(".openapi-parameter-add-value").trigger("click");
    await wrapper.get('[name="openapi-path:accountId-item-0"]').setValue("a/b");
    await wrapper.get(".openapi-parameter-add-value").trigger("click");
    await wrapper.get('[name="openapi-path:accountId-item-1"]').setValue("two");
    expect(wrapper.get<HTMLInputElement>('[name="openapi-request-url"]').element.value).toContain("/accounts/{accountId}");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(String(fetch.mock.calls[0]?.[0])).toContain("/accounts/;accountId=a%2Fb;accountId=two");
    wrapper.unmount();
  });
  it("shows only the selected API definition and its own contract details", () => {
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}" } });
    expect(wrapper.text()).toContain("List accounts");
    expect(wrapper.text()).toContain("Accounts returned");
    expect(wrapper.text()).not.toContain("2 Operations");
    wrapper.unmount();
  });

  it("edits a temporary request target while keeping the documented operation intact", async () => {
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", serviceId: "svc-api" } });
    expect(wrapper.find("#openapi-operation-documentation").exists()).toBe(true);
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");

    await wrapper.get("select[name='openapi-request-method']").setValue("POST");
    await wrapper.get("input[name='openapi-request-url']").setValue("https://api.example.test/accounts/42?limit=3");
    expect(wrapper.find("input[name='openapi-path:accountId']").exists()).toBe(false);
    expect(wrapper.get<HTMLInputElement>("input[name='openapi-query:limit']").element.value).toBe("3");
    await wrapper.get("#openapi-operation-documentation-tab").trigger("click");
    expect(wrapper.get(".openapi-operation-route-row").text()).toContain("GET");
    expect(wrapper.get(".openapi-operation-route-row").text()).toContain("/accounts/{accountId}");
    expect(wrapper.text()).not.toContain("保存请求");
    expect(wrapper.text()).not.toContain("Tests");
    wrapper.unmount();
  });

  it("aligns path and query parameter inputs with their type and description", async () => {
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", serviceId: "svc-api" } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    expect(wrapper.findAll(".debug-parameter-row")).toHaveLength(2);
    expect(wrapper.get(".debug-field-group").find('[aria-label="隐藏规范请求头"]').exists()).toBe(false);
    expect(wrapper.findAll<HTMLInputElement>(".debug-parameter-name input").map((row) => row.element.value)).toEqual(["limit", "accountId"]);
    expect(wrapper.findAll<HTMLInputElement>(".debug-parameter-description input").map((row) => row.element.value)).toContain("Account identifier");
    expect(wrapper.findAll<HTMLInputElement>(".debug-parameter-description input").map((row) => row.element.value)).toContain("Maximum number of accounts");
    expect(wrapper.findAll(".debug-type-select").map((item) => item.text())).toEqual(expect.arrayContaining(["string", "integer"]));
    wrapper.unmount();
  });

  it("displays the effective parameter example and preserves an explicitly cleared value", async () => {
    const example = content.replace("          description: Account identifier", "          example: '42'\n          description: Account identifier");
    const wrapper = mount(OpenApiPreview, { props: { content: example, focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    const input = wrapper.get<HTMLInputElement>('[name="openapi-path:accountId"]');
    expect(input.element.value).toBe("42");
    expect(wrapper.get<HTMLInputElement>('[name="openapi-request-url"]').element.value).toContain("/accounts/{accountId}");
    await input.setValue("");
    expect(input.element.value).toBe("");
    expect(wrapper.get<HTMLInputElement>('[name="openapi-request-url"]').element.value).not.toContain("/accounts/42");
    wrapper.unmount();
  });

  it("selects the actual response contract after receiving a non-success status", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response('{"message":"Missing"}', { status: 404, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetch);
    const withError = content + "        '404':\n          description: Not found\n          content:\n            application/json:\n              schema:\n                type: object\n                properties:\n                  message:\n                    type: string\n";
    const wrapper = mount(OpenApiPreview, { props: { content: withError, focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.get(".openapi-response-section").attributes("aria-busy")).toBe("false"));
    expect(wrapper.get<HTMLSelectElement>(".debug-response-actions select").element.value).toBe("404");
    expect(wrapper.findAll(".response-tabs [role='tab']").map((tab) => tab.text())).toEqual(["Body", "Cookie", "Header 1", "控制台", "实际请求"]);
    expect(wrapper.get(".response-panel-header").find(".response-metadata").exists()).toBe(true);
    expect(wrapper.get(".response-tabs").find(".response-metadata").exists()).toBe(false);
    await wrapper.get("#openapi-response-tab-cookies").trigger("click");
    expect(wrapper.get(".response-cookie-view").text()).toContain("Set-Cookie");
    await wrapper.get("#openapi-response-tab-console").trigger("click");
    expect(wrapper.get(".response-console").text()).toContain("没有内容");
    await wrapper.get("#openapi-response-tab-headers").trigger("click");
    expect(wrapper.get(".response-headers").text()).toContain("content-type");
    expect(wrapper.get(".response-data-table thead").text()).toContain("名称");
    await wrapper.get("#openapi-response-tab-request").trigger("click");
    expect(wrapper.get(".executed-request-view").text()).toContain("/accounts/42");
    wrapper.unmount();
  });

  it("retains a failed submitted request, exposes its console error, and clears diagnostics on a successful retry", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockRejectedValueOnce(new TypeError("Network unavailable"))
        .mockResolvedValueOnce(new Response("{}", { status: 200, headers: { "content-type": "application/json" } })),
    );
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.get(".openapi-response-section").attributes("aria-busy")).toBe("false"));
    expect(wrapper.get("#openapi-response-tab-body").text()).toContain("请求异常");
    expect(wrapper.find(".response-metadata").exists()).toBe(false);
    await wrapper.get("#openapi-response-tab-console").trigger("click");
    expect(wrapper.get(".response-console").text()).toContain("Network unavailable");
    await wrapper.get("#openapi-response-tab-request").trigger("click");
    expect(wrapper.get(".executed-request-view").text()).toContain("/accounts/42");
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.get(".openapi-response-section").attributes("aria-busy")).toBe("false"));
    expect(wrapper.find(".openapi-request-error").exists()).toBe(false);
    expect(wrapper.get("#openapi-response-tab-body").text()).toBe("Body");
    expect(wrapper.get(".response-metadata").text()).toContain("200");
    await wrapper.get("#openapi-response-tab-console").trigger("click");
    expect(wrapper.get(".response-console").text()).toContain("没有内容");
    wrapper.unmount();
  });

  it("provides separate body and script sections without a tests section", async () => {
    const bodyContent = content.replace("      responses:\n", "      requestBody:\n        content:\n          application/json:\n            schema:\n              type: object\n              properties:\n                name:\n                  type: string\n      responses:\n");
    const wrapper = mount(OpenApiPreview, { props: { content: bodyContent, focusOperationKey: "GET:/accounts/{accountId}", serviceId: "svc-api" } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    expect(wrapper.find("#openapi-debug-section-tab-body").exists()).toBe(true);
    expect(wrapper.find("#openapi-debug-section-tab-before").exists()).toBe(true);
    await wrapper.get("#openapi-debug-section-tab-before").trigger("click");
    await wrapper.get('[data-phase="before"] .openapi-script-sequence-footer button').trigger("click");
    expect(wrapper.get('[data-phase="before"]').element.parentElement?.style.display).toBe("");
    expect(wrapper.get('[data-phase="after"]').element.parentElement?.style.display).toBe("none");
    await wrapper.get("#openapi-debug-section-tab-after").trigger("click");
    expect(wrapper.get("#openapi-debug-section-tab-after").attributes("aria-selected")).toBe("true");
    expect(wrapper.get('[data-phase="after"]').element.parentElement?.style.display).toBe("");
    expect(wrapper.get('[data-phase="before"]').element.parentElement?.style.display).toBe("none");
    expect(wrapper.text()).not.toContain("添加断言");
    wrapper.unmount();
  });

  it("sends edited parameters with the selected redirect setting and displays the response", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response('{"ok":true}', { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetch);
    const timer = vi.spyOn(window, "setTimeout");
    const wrapper = mount(OpenApiPreview, { props: { content, focusOperationKey: "GET:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get("input[name='openapi-path:accountId']").setValue("42");
    await wrapper.get("input[name='openapi-query:limit']").setValue("5");
    await wrapper.get("#openapi-debug-section-tab-settings").trigger("click");
    await wrapper.get("input[name='openapi-follow-redirects']").setValue(false);
    await wrapper.get("input[name='openapi-request-timeout']").setValue(7);
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.get(".openapi-response-section").attributes("aria-busy")).toBe("false"));
    expect(fetch).toHaveBeenCalledWith("https://api.example.test/accounts/42?limit=5", expect.objectContaining({ method: "GET", redirect: "error", credentials: "omit" }));
    expect(timer).toHaveBeenCalledWith(expect.any(Function), 7000);
    expect(wrapper.get(".response-metadata").text()).toContain("200");
    expect(wrapper.find(".openapi-response-empty").exists()).toBe(false);
    wrapper.unmount();
  });

  it("does not send hidden JSON after choosing no body", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const wrapper = mount(OpenApiPreview, { props: { content: content.replace("    get:", "    post:"), focusOperationKey: "POST:/accounts/{accountId}", publicTryIt: true } });
    await wrapper.get("#openapi-operation-debug-tab").trigger("click");
    await wrapper.get('[name="openapi-path:accountId"]').setValue("42");
    await wrapper.get("#openapi-debug-section-tab-body").trigger("click");
    await wrapper.get('[name="openapi-body-format"][value="application/json"]').setValue();
    await wrapper.get(".openapi-payload-editor textarea").setValue('{"name":"Cat"}');
    await wrapper.get('[name="openapi-body-format"][value=""]').setValue();
    await wrapper.get(".request-target-send-button").trigger("click");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.get(".openapi-response-section").attributes("aria-busy")).toBe("false"));
    expect(fetch).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({ body: undefined }));
    wrapper.unmount();
  });
});
