import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import OpenApiPublicationDialog from "./OpenApiPublicationDialog.vue";
import { ApiError } from "@/lib/backend/types";
import type { Specification } from "@/types/domain";
import type { OpenApiPublicationPreview } from "@/types/api";

const api = vi.hoisted(() => ({
  listOpenApiPublications: vi.fn(),
  listSpecificationRevisions: vi.fn(),
  previewOpenApiPublication: vi.fn(),
  publishOpenApi: vi.fn(),
  revokeOpenApiPublication: vi.fn(),
}));
vi.mock("@/lib/backend/client", () => ({ specxApi: api }));
const specification: Specification = { id: "spec", serviceId: "service", title: "Pets", kind: "openapi", version: 3, content: "{}", contentHash: "hash", etag: "etag", updatedAt: "2026-11-11T00:00:00Z" };
const publication = { id: "a".repeat(32), version: 2, title: "Stable pets", description: "Public API", theme: "dark" as const, showDownload: false, allowTryIt: true, createdAt: "2026-11-11T00:00:00Z" };
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}
function setup() {
  return mount(OpenApiPublicationDialog, {
    props: { workspaceId: "workspace", specification },
    global: {
      stubs: {
        Dialog: { template: "<div><slot /></div>" },
        DialogScrollContent: { template: "<div><slot /></div>" },
        DialogHeader: { template: "<div><slot /></div>" },
        DialogTitle: { template: "<h2><slot /></h2>" },
        DialogDescription: true,
        DialogFooter: { template: "<div><slot /></div>" },
        OpenApiDocumentReader: { props: ["content"], template: "<div data-preview>{{ content }}</div>" },
      },
    },
  });
}
type Wrapper = ReturnType<typeof setup>;
function button(wrapper: Wrapper, label: string) {
  return wrapper.findAll("button").find((entry) => entry.text() === label)!;
}
beforeEach(() => {
  vi.resetAllMocks();
  api.listOpenApiPublications.mockResolvedValue({ items: [], nextOffset: null });
  api.listSpecificationRevisions.mockResolvedValue({ items: [{ version: 3 }, { version: 2 }], nextOffset: null });
  api.previewOpenApiPublication.mockResolvedValue({ content: "current revision", digest: "current-digest" });
  api.publishOpenApi.mockResolvedValue(publication);
  api.revokeOpenApiPublication.mockResolvedValue(undefined);
});

describe("OpenAPI publication review", () => {
  it("copies and reviews only ordinary variables and publishes the explicit environment snapshot", async () => {
    const previewContent = JSON.stringify({ openapi: "3.1.0", info: { title: "Demo", version: "1" }, paths: {} });
    api.previewOpenApiPublication.mockResolvedValue({ content: previewContent, digest: "reviewed" });
    const wrapper = setup();
    await wrapper.setProps({
      environments: [
        {
          id: "demo",
          serviceId: "service",
          slug: "demo",
          createdAt: "2026-11-12T00:00:00Z",
          updatedAt: "2026-11-12T00:00:00Z",
          name: "Demo",
          baseUrl: "https://{{host}}",
          variables: [
            { key: "host", value: "jsonplaceholder.typicode.com", enabled: true, sensitive: false, hasValue: true },
            { key: "hidden", value: "private-value", enabled: true, sensitive: true, hasValue: true },
            { key: "disabled", value: "disabled-value", enabled: false, sensitive: false, hasValue: true },
            { key: "secret", value: null, enabled: true, sensitive: true, hasValue: true },
          ],
        },
      ],
    });
    await flushPromises();
    await button(wrapper, "新建分享").trigger("click");
    await flushPromises();
    await wrapper.get('select[name="public-environment-source"]').setValue("demo");
    await button(wrapper, "添加环境").trigger("click");
    const editor = wrapper.get(".public-environment-editor");
    expect(editor.findAll("input").map((entry) => entry.element.value)).toEqual(["Demo", "https://{{host}}", "host", "jsonplaceholder.typicode.com"]);
    await editor.get('input[name$="-url"]').setValue("https://{{missing}}");
    expect(button(wrapper, "发布分享链接").attributes("disabled")).toBeDefined();
    await editor.get('input[name$="-url"]').setValue("https://{{host}}");
    await button(wrapper, "发布分享链接").trigger("click");
    await flushPromises();
    expect(api.publishOpenApi).toHaveBeenCalledWith("workspace", "spec", expect.objectContaining({ environments: [{ name: "Demo", baseUrl: "https://{{host}}", variables: [{ key: "host", value: "jsonplaceholder.typicode.com" }] }] }));
    expect(JSON.stringify(api.publishOpenApi.mock.calls)).not.toMatch(/private-value|disabled-value|secret-id/);
    wrapper.unmount();
  });

  it("ignores a late preview and freezes the reviewed revision while publishing", async () => {
    const old = deferred<OpenApiPublicationPreview>();
    const latest = deferred<OpenApiPublicationPreview>();
    const publishing = deferred<typeof publication>();
    api.previewOpenApiPublication.mockReturnValueOnce(old.promise).mockReturnValueOnce(latest.promise);
    api.publishOpenApi.mockReturnValueOnce(publishing.promise);
    const wrapper = setup();
    await flushPromises();
    await button(wrapper, "新建分享").trigger("click");
    await flushPromises();
    await wrapper.get("select").setValue("2");
    latest.resolve({ content: "reviewed revision", digest: "reviewed-digest" });
    await flushPromises();
    old.resolve({ content: "stale revision", digest: "stale-digest" });
    await flushPromises();
    expect(wrapper.get("[data-preview]").text()).toBe("reviewed revision");
    await wrapper.get('input[name="openapi-publication-title"]').setValue("Stable pets");
    await button(wrapper, "发布分享链接").trigger("click");
    expect(wrapper.get("select").attributes("disabled")).toBeDefined();
    expect(wrapper.get('input[name="openapi-publication-title"]').attributes("disabled")).toBeDefined();
    expect(button(wrapper, "发布分享链接").attributes("aria-busy")).toBe("true");
    await button(wrapper, "发布分享链接").trigger("click");
    expect(api.publishOpenApi).toHaveBeenCalledExactlyOnceWith("workspace", "spec", { title: "Stable pets", description: "", version: 2, theme: "light", showDownload: true, allowTryIt: true, digest: "reviewed-digest" });
    publishing.resolve(publication);
    await flushPromises();
    expect(wrapper.text()).toContain("分享链接已创建");
    expect(wrapper.get("a").attributes("href")).toContain(`${publication.id}.localhost`);
    wrapper.unmount();
  });

  it("retains review and revoke state after failures, and cancellation never revokes", async () => {
    api.publishOpenApi.mockRejectedValueOnce(new ApiError("Offline", 503));
    api.revokeOpenApiPublication.mockRejectedValueOnce(new ApiError("Offline", 503));
    const wrapper = setup();
    await flushPromises();
    await button(wrapper, "新建分享").trigger("click");
    await flushPromises();
    await button(wrapper, "发布分享链接").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe("Offline");
    expect(wrapper.get("[data-preview]").text()).toBe("current revision");
    await button(wrapper, "发布分享链接").trigger("click");
    await flushPromises();
    await button(wrapper, "撤销分享").trigger("click");
    await button(wrapper, "取消").trigger("click");
    expect(api.revokeOpenApiPublication).not.toHaveBeenCalled();
    await button(wrapper, "撤销分享").trigger("click");
    await button(wrapper, "撤销分享").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe("Offline");
    expect(wrapper.text()).toContain("Stable pets");
    await button(wrapper, "撤销分享").trigger("click");
    await flushPromises();
    expect(api.revokeOpenApiPublication).toHaveBeenLastCalledWith("workspace", "spec", publication.id);
    expect(wrapper.text()).toContain("这份规范还没有分享链接");
    wrapper.unmount();
  });

  it("blocks publication after a rejected preview and allows a failed manager load to retry", async () => {
    api.listOpenApiPublications.mockRejectedValueOnce(new ApiError("Offline", 503));
    api.previewOpenApiPublication.mockRejectedValueOnce(new ApiError("Rejected", 422));
    const wrapper = setup();
    await flushPromises();
    expect(button(wrapper, "新建分享").attributes("disabled")).toBeDefined();
    await button(wrapper, "重试").trigger("click");
    await flushPromises();
    await button(wrapper, "新建分享").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain("移除示例");
    expect(button(wrapper, "发布分享链接").attributes("disabled")).toBeDefined();
    expect(api.publishOpenApi).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
