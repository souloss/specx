import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDebugBody from "./OpenApiDebugBody.vue";

describe("Body format navigation", () => {
  it("has one format chooser and includes document-specific media types", async () => {
    const wrapper = mount(OpenApiDebugBody, { props: { modelValue: "application/vnd.example+json", availableMediaTypes: ["application/vnd.example+json"] }, slots: { tools: '<button type="button">Bulk</button>', default: '<input aria-label="Body value">' } });
    expect(wrapper.find("select").exists()).toBe(false);
    expect(wrapper.get<HTMLInputElement>('[name="openapi-body-format"][value="application/vnd.example+json"]').element.checked).toBe(true);
    expect(wrapper.get(".debug-body-content-toolbar").text()).toContain("Bulk");
    await wrapper.get('[name="openapi-body-format"][value="text/html"]').setValue(true);
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual(["text/html"]);
    wrapper.unmount();
  });
});
