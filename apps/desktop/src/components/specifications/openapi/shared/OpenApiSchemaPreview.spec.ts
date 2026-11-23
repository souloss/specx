import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiSchemaPreview from "./OpenApiSchemaPreview.vue";

describe("schema field navigation", () => {
  it("distinguishes defaults from examples and exposes nested formats and exclusive bounds", () => {
    const wrapper = mount(OpenApiSchemaPreview, { props: { document: null, schema: { type: "object", minProperties: 1, properties: { count: { type: "integer", format: "int64", default: 0, example: 2, exclusiveMinimum: 0, maximum: 10, exclusiveMaximum: true } } } } });
    expect(wrapper.text()).toContain("默认值0");
    expect(wrapper.text()).toContain("示例2");
    expect(wrapper.text()).toContain("integer · int64");
    expect(wrapper.text()).toContain("不含最小值 0");
    expect(wrapper.text()).toContain("不含最大值 10");
    expect(wrapper.text()).toContain("最少属性数 1");
    wrapper.unmount();
  });
  it("shows array item and map value types even in compact parameter documentation", () => {
    const wrapper = mount(OpenApiSchemaPreview, { props: { document: null, schema: { type: "object", additionalProperties: { type: "array", items: { type: "string" } } }, compact: true } });
    expect(wrapper.text()).toContain("数组<string>");
    expect(wrapper.text()).toContain("数组元素 string");
    wrapper.unmount();
  });
  it("keeps recursive fields bounded and offers keyboard-native reference navigation", async () => {
    const document = { components: { schemas: { Node: { type: "object", properties: { left: { $ref: "#/components/schemas/Node" }, right: { $ref: "#/components/schemas/Node" } } } } } };
    const navigateModel = vi.fn();
    const wrapper = mount(OpenApiSchemaPreview, { props: { document, schema: { $ref: "#/components/schemas/Node" }, navigateModel } });
    expect(wrapper.findAll(".schema-preview")).toHaveLength(3);
    expect(wrapper.text()).toContain("left");
    expect(wrapper.text()).toContain("递归引用");
    await wrapper.find('button[aria-label="查看模型 Node"]').trigger("click");
    expect(navigateModel).toHaveBeenCalledWith("#/components/schemas/Node");
    wrapper.unmount();
  });
  it("renders boolean schemas distinctly, and remains read only without a navigation handler", () => {
    const wrapper = mount(OpenApiSchemaPreview, { props: { document: { components: { schemas: { Never: false } } }, schema: { $ref: "#/components/schemas/Never" } } });
    expect(wrapper.text()).toContain("不允许任何值");
    expect(wrapper.find('button[aria-label="查看模型 Never"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
