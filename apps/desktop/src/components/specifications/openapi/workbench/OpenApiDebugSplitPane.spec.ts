import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDebugSplitPane from "./OpenApiDebugSplitPane.vue";

describe("debug response resizing", () => {
  it("supports keyboard resizing, clamps the panels and restores the default", async () => {
    const wrapper = mount(OpenApiDebugSplitPane, { slots: { request: "Request fields", response: "Response body" } });
    const handle = wrapper.get('[role="separator"]');
    await handle.trigger("keydown", { key: "ArrowDown" });
    expect(handle.attributes("aria-valuenow")).toBe("51");
    await handle.trigger("keydown", { key: "Home" });
    await handle.trigger("keydown", { key: "ArrowUp" });
    expect(handle.attributes("aria-valuenow")).toBe("20");
    await handle.trigger("keydown", { key: "End" });
    expect(handle.attributes("aria-valuenow")).toBe("85");
    await handle.trigger("dblclick");
    expect(handle.attributes("aria-valuenow")).toBe("46");
    expect(wrapper.text()).toContain("Response body");
    wrapper.unmount();
  });
});
