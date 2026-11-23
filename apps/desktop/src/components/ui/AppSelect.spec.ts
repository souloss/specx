import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import AppSelect from "./AppSelect.vue";

describe("shared select control", () => {
  it("gives only the visible trigger the public id and accessible label", () => {
    const wrapper = mount(AppSelect, { props: { id: "language", modelValue: "go", name: "language" }, attrs: { "aria-label": "Language", class: "w-full" }, slots: { default: '<option value="go">Go</option>' } });
    expect(wrapper.findAll("#language")).toHaveLength(1);
    expect(wrapper.get('button[role="combobox"]').attributes("aria-label")).toBe("Language");
    expect(wrapper.get('button[role="combobox"]').attributes("data-app-select-size")).toBe("compact");
    expect(wrapper.get("button").classes()).toContain("w-full");
    expect(wrapper.get("select").attributes("aria-hidden")).toBe("true");
    expect(wrapper.get("select").attributes("aria-label")).toBeUndefined();
    expect(wrapper.get("select").attributes("name")).toBe("language");
    wrapper.unmount();
  });

  it("preserves numeric and boolean models and dispatches the native change event", async () => {
    const wrapper = mount(
      defineComponent({
        components: { AppSelect },
        data: () => ({ value: 1, checked: false }),
        template: '<AppSelect v-model.number="value" @change="checked = true"><option :value="1">One</option><option :value="2">Two</option></AppSelect><span data-result>{{ typeof value }}:{{ value }}:{{ checked }}</span>',
      }),
    );
    await wrapper.get("select").setValue("2");
    expect(wrapper.get("[data-result]").text()).toBe("number:2:true");
    wrapper.unmount();
    const boolean = mount(AppSelect, { props: { modelValue: false }, slots: { default: '<option :value="false">No</option><option :value="true">Yes</option>' } });
    await boolean.get("select").setValue("true");
    expect(boolean.emitted("update:modelValue")?.[0]).toEqual([true]);
    boolean.unmount();
  });

  it("disables both visible interaction and native form submission", () => {
    const wrapper = mount(AppSelect, { props: { disabled: true, modelValue: "go" }, slots: { default: '<option value="go">Go</option>' } });
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
    expect(wrapper.get("select").attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("exposes shared size and appearance variants on the visible control", () => {
    const wrapper = mount(AppSelect, {
      props: { modelValue: "json", size: "dense", variant: "quiet" },
      slots: { default: '<option value="json">JSON</option>' },
    });

    expect(wrapper.get('button[role="combobox"]').attributes("data-app-select-size")).toBe("dense");
    expect(wrapper.get('button[role="combobox"]').attributes("data-app-select-variant")).toBe("quiet");
    wrapper.unmount();
  });
});
