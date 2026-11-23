import { describe, it, expect, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import OpenApiVariableInput from "./OpenApiVariableInput.vue";
afterEach(() => {
  document.body.innerHTML = "";
});
describe("dynamic variable input", () => {
  it("filters completion after {{$ and inserts by keyboard without submitting the request", async () => {
    const wrapper = mount(OpenApiVariableInput, { attachTo: document.body, attrs: { "aria-label": "Value" }, props: { modelValue: "", "onUpdate:modelValue": (value) => void wrapper.setProps({ modelValue: value }) } });
    const input = wrapper.get("input");
    expect(wrapper.find(".dynamic-value-trigger").exists()).toBe(true);
    await input.setValue("prefix {{$string.uu");
    await flushPromises();
    expect(input.attributes("role")).toBe("combobox");
    expect(document.querySelector("[role=listbox]")?.textContent).toContain("$string.uuid");
    await input.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(wrapper.props("modelValue")).toBe("prefix {{$string.uuid}}");
    expect(input.attributes("aria-expanded")).toBe("false");
    wrapper.unmount();
  });
  it("closes suggestions with Escape and preserves an unfinished expression", async () => {
    const wrapper = mount(OpenApiVariableInput, { attachTo: document.body, props: { modelValue: "", "onUpdate:modelValue": (value) => void wrapper.setProps({ modelValue: value }) } });
    await wrapper.get("input").setValue("{{$number");
    await flushPromises();
    await wrapper.get("input").trigger("keydown", { key: "Escape" });
    expect(wrapper.props("modelValue")).toBe("{{$number");
    wrapper.unmount();
  });
});
