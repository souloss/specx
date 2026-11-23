import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiSchemaModelPicker from "./OpenApiSchemaModelPicker.vue";

const document = { openapi: "3.1.0", components: { schemas: { Pet: { type: "object", properties: { name: { type: "string", description: "Display name" } } }, Error: { type: "object", properties: { message: { type: "string" } } }, Never: false } } };
function setup(currentReference?: string) {
  return mount(OpenApiSchemaModelPicker, {
    props: { open: true, document, currentReference, label: "Response" },
    global: { stubs: { Dialog: { template: "<div><slot /></div>" }, DialogScrollContent: { template: "<div><slot /></div>" }, DialogHeader: { template: "<div><slot /></div>" }, DialogTitle: true, DialogDescription: true, DialogFooter: { template: "<div><slot /></div>" } } },
  });
}
describe("schema model selection", () => {
  it("previews full fields and keeps the current selection while searching by fields", async () => {
    const wrapper = setup("#/components/schemas/Pet");
    expect(wrapper.find('input[aria-label="Pet"]').element).toHaveProperty("checked", true);
    expect(wrapper.find(".schema-model-picker-preview").text()).toContain("Display name");
    await wrapper.find('input[name="openapi-model-search"]').setValue("message");
    expect(wrapper.findAll('input[type="radio"]')).toHaveLength(1);
    expect(wrapper.find('input[type="radio"]').attributes("aria-label")).toBe("Error");
    await wrapper.find('input[type="radio"]').setValue(true);
    expect(wrapper.find(".schema-model-picker-preview").text()).toContain("message");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "使用此引用")!
      .trigger("click");
    expect(wrapper.emitted("select")).toEqual([["#/components/schemas/Error"]]);
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
    wrapper.unmount();
  });

  it("supports boolean model previews and cancellation without changing the contract", async () => {
    const wrapper = setup();
    await wrapper.find('input[aria-label="Never"]').setValue(true);
    expect(wrapper.find(".schema-model-picker-preview").text()).toContain("不允许任何值");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "取消")!
      .trigger("click");
    expect(wrapper.emitted("select")).toBeUndefined();
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
    wrapper.unmount();
  });

  it("keeps confirmation disabled when the selected model was removed remotely", async () => {
    const wrapper = setup("#/components/schemas/Pet");
    await wrapper.setProps({ document: { openapi: "3.1.0", components: { schemas: {} } } });
    expect(wrapper.text()).toContain("请先在侧边栏");
    expect(
      wrapper
        .findAll("button")
        .find((button) => button.text() === "使用此引用")!
        .attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.emitted("select")).toBeUndefined();
    wrapper.unmount();
  });
});
