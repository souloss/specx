import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDebugResponsePanel from "./OpenApiDebugResponsePanel.vue";

describe("debug response panel", () => {
  it("selects a declared response status and toggles contract validation", async () => {
    const wrapper = mount(OpenApiDebugResponsePanel, {
      props: {
        running: false,
        hasResponse: false,
        statusOptions: [
          { code: "200", label: "Success" },
          { code: "404", label: "Not found" },
        ],
        status: "200",
        validate: false,
      },
    });
    await wrapper.get("select").setValue("404");
    expect(wrapper.emitted("update:status")?.at(-1)).toEqual(["404"]);
    await wrapper.get('input[name="openapi-validate-response"]').setValue(true);
    expect(wrapper.emitted("update:validate")?.at(-1)).toEqual([true]);
    wrapper.unmount();
  });

  it("keeps a successful response at full width and expands only failed validation", async () => {
    const wrapper = mount(OpenApiDebugResponsePanel, {
      props: { running: false, hasResponse: true, validate: true, statusOptions: [{ code: "200", label: "Success" }], validation: { ok: true, errors: [] } },
    });
    expect(wrapper.find(".response-validation-sidebar").exists()).toBe(false);
    expect(wrapper.find(".response-validation-result.success").exists()).toBe(true);
    await wrapper.setProps({ validation: { ok: false, errors: ["$.name is required"] } });
    expect(wrapper.get(".response-validation-sidebar").text()).toContain("$.name is required");
    expect(wrapper.get('[role="separator"]').attributes("aria-orientation")).toBe("vertical");
    await wrapper.get('[role="separator"]').trigger("keydown", { key: "ArrowLeft" });
    expect(wrapper.get('[role="separator"]').attributes("aria-valuenow")).toBe("33");
    await wrapper.get(".response-validation-result.failure").trigger("click");
    expect(wrapper.find(".response-validation-sidebar").exists()).toBe(false);
    await wrapper.get(".response-validation-result.failure").trigger("click");
    expect(wrapper.find(".response-validation-sidebar").exists()).toBe(true);
    await wrapper.setProps({ validate: false });
    expect(wrapper.find(".response-validation-sidebar").exists()).toBe(false);
    wrapper.unmount();
  });
});
