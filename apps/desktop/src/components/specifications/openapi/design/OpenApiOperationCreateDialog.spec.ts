import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import OpenApiOperationCreateDialog from "./OpenApiOperationCreateDialog.vue";

const content = JSON.stringify({ openapi: "3.0.3", info: { title: "Operations", version: "1" }, paths: { "/pets": { get: { responses: { "200": { description: "OK" } } } } } });
function setup(save = vi.fn(async (_content: string, _change?: OpenApiOperationChange) => true)) {
  const wrapper = mount(OpenApiOperationCreateDialog, {
    props: { open: true, content, editable: true, saveOperation: save },
    global: { stubs: { Dialog: { template: "<div><slot /></div>" }, DialogScrollContent: { template: "<div><slot /></div>" }, DialogHeader: true, DialogTitle: true, DialogFooter: { template: "<div><slot /></div>" } } },
  });
  return { wrapper, save };
}

describe("OpenAPI API creation", () => {
  it("validates duplicates before saving and creates a tagged API with its navigation identity", async () => {
    const { wrapper, save } = setup();
    await wrapper.find('input[name="openapi-create-path"]').setValue("/pets");
    await wrapper.find("form").trigger("submit");
    expect(save).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("此路径和方法已存在");
    await wrapper.find('input[name="openapi-create-path"]').setValue("/orders/{orderId}");
    await wrapper.find('input[name="openapi-create-summary"]').setValue("Read an order");
    await wrapper.find('input[name="openapi-create-tags"]').setValue("Orders");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toMatchObject({
      paths: { "/pets": { get: { responses: { "200": { description: "OK" } } } }, "/orders/{orderId}": { get: { summary: "Read an order", tags: ["Orders"], parameters: [{ name: "orderId", in: "path", required: true, schema: { type: "string" } }] } } },
    });
    expect(save.mock.calls[0]![1]).toEqual({ key: "GET:/orders/{orderId}", title: "GET /orders/{orderId}" });
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
    wrapper.unmount();
  });

  it("preserves entered metadata when saving fails and supports retry", async () => {
    const save = vi.fn(async (_content: string, _change?: OpenApiOperationChange) => true).mockRejectedValueOnce(new Error("Network unavailable"));
    const { wrapper } = setup(save);
    await wrapper.find('input[name="openapi-create-path"]').setValue("/orders");
    await wrapper.find('input[name="openapi-create-summary"]').setValue("Orders");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("Network unavailable");
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-create-summary"]').element.value).toBe("Orders");
    expect(wrapper.emitted("update:open")).toBeUndefined();
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(save).toHaveBeenCalledTimes(2);
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
    wrapper.unmount();
  });
});
