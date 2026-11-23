import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import EnvironmentEditor from "./EnvironmentEditor.vue";

function mountEditor() {
  return mount(EnvironmentEditor, { attachTo: document.body, props: { modelValue: { id: "qa", name: "", slug: "qa", requestMode: "inherit", baseUrl: "", variables: [] }, busy: false } });
}
describe("environment editor validation", () => {
  it("focuses invalid fields and retains the draft until corrected", async () => {
    const wrapper = mountEditor();
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get('[name="environment-name"]').element);
    expect(wrapper.get('[role="alert"]').text()).toContain("环境名称");
    await wrapper.get('[name="environment-name"]').setValue("QA");
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    await wrapper.get('[name="environment-base-url"]').setValue("file:///tmp/qa");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get('[name="environment-base-url"]').element);
    expect(wrapper.emitted("save")).toBeUndefined();
    await wrapper.get('[name="environment-base-url"]').setValue("https://{{host}}/v1");
    await wrapper.get("form").trigger("submit");
    expect(wrapper.emitted("save")).toEqual([[false]]);
    wrapper.unmount();
  });
  it("rejects duplicate variables and masks retained sensitive values", async () => {
    const wrapper = mountEditor();
    await wrapper.setProps({
      modelValue: {
        id: "qa",
        name: "QA",
        slug: "qa",
        requestMode: "inherit",
        baseUrl: "https://qa.test",
        variables: [
          { key: "region", value: "west", sensitive: false, enabled: true },
          { key: "region", value: "", sensitive: true, enabled: true, hasValue: true },
        ],
      },
    });
    expect(wrapper.get('[name="environment-value-1"]').attributes("type")).toBe("password");
    expect(wrapper.get('[name="environment-value-1"]').attributes("placeholder")).toContain("保留");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get('[name="environment-key-1"]').element);
    expect(wrapper.emitted("save")).toBeUndefined();
    wrapper.unmount();
  });
});
