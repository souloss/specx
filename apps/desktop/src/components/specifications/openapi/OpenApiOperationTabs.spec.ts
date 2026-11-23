import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import OpenApiOperationTabs from "./OpenApiOperationTabs.vue";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("operation tab navigation", () => {
  it("selects and focuses tabs with arrow keys and boundary keys", async () => {
    const wrapper = mount(OpenApiOperationTabs, {
      attachTo: document.body,
      props: {
        modelValue: "params",
        ariaLabel: "Request sections",
        tabs: [
          { value: "params", id: "request-params", label: "Params" },
          { value: "body", id: "request-body", label: "Body" },
          { value: "settings", id: "request-settings", label: "Settings" },
        ],
        "onUpdate:modelValue": (modelValue: string) => void wrapper.setProps({ modelValue }),
      },
    });
    const params = wrapper.get<HTMLButtonElement>("#request-params");
    params.element.focus();
    await params.trigger("keydown", { key: "ArrowLeft" });
    expect(document.activeElement?.id).toBe("request-settings");
    expect(wrapper.get("#request-settings").attributes("aria-selected")).toBe("true");
    await wrapper.get("#request-settings").trigger("keydown", { key: "Home" });
    expect(document.activeElement?.id).toBe("request-params");
    await params.trigger("keydown", { key: "ArrowRight" });
    expect(document.activeElement?.id).toBe("request-body");
    await wrapper.get("#request-body").trigger("keydown", { key: "End" });
    expect(document.activeElement?.id).toBe("request-settings");
    expect(wrapper.findAll('[tabindex="0"]')).toHaveLength(1);
    wrapper.unmount();
  });
});
