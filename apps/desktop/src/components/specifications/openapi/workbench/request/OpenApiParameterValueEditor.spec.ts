import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiParameterValueEditor from "./OpenApiParameterValueEditor.vue";

describe("OpenAPI parameter value editor", () => {
  it("adds and removes array items while preserving typed values and item constraints", async () => {
    const wrapper = mount(OpenApiParameterValueEditor, {
      props: {
        modelValue: '["first"]',
        name: "query:ids",
        schema: { type: "array", minItems: 1, maxItems: 2, items: { type: "integer" } },
      },
    });

    await wrapper.find(".openapi-parameter-add-value").trigger("click");
    let nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual(["first", ""]);
    await wrapper.setProps({ modelValue: nextValue });

    await wrapper.find<HTMLInputElement>('input[name="query:ids-item-1"]').setValue("7");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual(["first", 7]);
    await wrapper.setProps({ modelValue: nextValue });
    expect(wrapper.find(".openapi-parameter-add-value").exists()).toBe(false);

    await wrapper.find<HTMLButtonElement>('[aria-label="删除第 2 项"]').trigger("click");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual(["first"]);
    await wrapper.setProps({ modelValue: nextValue });
    expect(wrapper.find<HTMLButtonElement>('[aria-label="删除第 1 项"]').element.disabled).toBe(true);
  });

  it("adds schema properties with typed controls and keeps required properties", async () => {
    const wrapper = mount(OpenApiParameterValueEditor, {
      props: {
        modelValue: "{}",
        name: "query:filter",
        schema: {
          type: "object",
          required: ["role"],
          properties: { role: { type: "string" }, active: { type: "boolean" } },
          additionalProperties: { type: "number" },
        },
      },
    });

    await wrapper.find(".openapi-parameter-add-value").trigger("click");
    let nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual({ role: "" });
    await wrapper.setProps({ modelValue: nextValue });
    expect(wrapper.find<HTMLButtonElement>('[aria-label="删除属性 role"]').element.disabled).toBe(true);

    await wrapper.find(".openapi-parameter-add-value").trigger("click");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual({ role: "", active: "" });
    await wrapper.setProps({ modelValue: nextValue });
    await wrapper.find<HTMLSelectElement>('select[name="query:filter-value-active"]').setValue("true");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual({ role: "", active: true });
    await wrapper.setProps({ modelValue: nextValue });

    await wrapper.find(".openapi-parameter-add-value").trigger("click");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual({ role: "", active: true, property1: "" });
    await wrapper.setProps({ modelValue: nextValue });
    await wrapper.find<HTMLInputElement>('input[name="query:filter-value-property1"]').setValue("2.5");
    nextValue = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as string;
    expect(JSON.parse(nextValue)).toEqual({ role: "", active: true, property1: 2.5 });

    const restricted = mount(OpenApiParameterValueEditor, {
      props: {
        modelValue: '{"role":"reader"}',
        name: "query:restricted",
        schema: { type: "object", properties: { role: { type: "string" } }, additionalProperties: false },
      },
    });
    expect(restricted.find(".openapi-parameter-add-value").exists()).toBe(false);
  });

  it("renders boolean choices and accepts variable placeholders in numeric parameters", async () => {
    const booleanEditor = mount(OpenApiParameterValueEditor, {
      props: { modelValue: "", name: "query:active", schema: { type: "boolean" } },
    });
    await booleanEditor.find<HTMLSelectElement>('select[name="query:active"]').setValue("false");
    expect(booleanEditor.emitted("update:modelValue")?.at(-1)?.[0]).toBe("false");

    const numberEditor = mount(OpenApiParameterValueEditor, {
      props: { modelValue: "", name: "query:count", schema: { type: "integer" } },
    });
    expect(numberEditor.find<HTMLInputElement>('input[name="query:count"]').attributes("type")).toBe("text");
    await numberEditor.get('input[name="query:count"]').setValue("{{count}}");
    expect(numberEditor.emitted("update:modelValue")?.at(-1)?.[0]).toBe("{{count}}");
  });
});
