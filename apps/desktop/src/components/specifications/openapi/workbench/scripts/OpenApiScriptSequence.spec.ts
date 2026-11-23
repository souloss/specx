import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiScriptSequence from "./OpenApiScriptSequence.vue";

describe("OpenAPI script sequence", () => {
  it("adds, reorders, removes and appends snippets without collapsing phases", async () => {
    const wrapper = mount(OpenApiScriptSequence, {
      props: {
        modelValue: ["first()", "second()"],
        phase: "before",
        label: "Pre-request",
        addLabel: "Add step",
        helpLabel: "Help",
        moveUpLabel: "Move up",
        moveDownLabel: "Move down",
        removeLabel: "Delete",
        snippetsLabel: "Snippets",
        snippets: [{ label: "Log", value: "console.log(1);" }],
      },
    });

    await wrapper.findAll(".openapi-script-item")[1]!.find('button[aria-label="Pre-request 2 Move up"]').trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toEqual(["second()", "first()"]);
    await wrapper.setProps({ modelValue: ["second()", "first()"] });

    await wrapper.findAll(".openapi-script-snippet")[0]!.trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toEqual(["second()\n\nconsole.log(1);", "first()"]);

    await wrapper.find('button[aria-label="Pre-request 1 Delete"]').trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toEqual(["first()"]);
    wrapper.unmount();
  });
});
