// @vitest-environment happy-dom

import { createApp, defineComponent, h, nextTick } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import AppTooltip from "@/components/ui/AppTooltip.vue";

afterEach(() => {
  document.body.replaceChildren();
  vi.useRealTimers();
});

describe("AppTooltip", () => {
  it("does not block a click after the tooltip has opened", async () => {
    vi.useFakeTimers();
    const clicked = vi.fn();
    const root = defineComponent({
      setup: () => () => h(AppTooltip, { text: "Settings" }, { default: () => h("button", { id: "settings", onClick: clicked }, "Settings") }),
    });
    const container = document.createElement("div");
    document.body.append(container);
    const app = createApp(root);
    app.mount(container);

    const trigger = container.querySelector<HTMLElement>("#settings");
    expect(trigger).toBeTruthy();
    vi.spyOn(trigger!, "matches").mockImplementation((selector) => selector === ":hover");
    trigger?.dispatchEvent(new MouseEvent("mouseenter", { bubbles: true }));
    await vi.runAllTimersAsync();
    await nextTick();
    expect(document.querySelector('[role="tooltip"]')).toBeTruthy();

    trigger?.click();
    document.dispatchEvent(new Event("pointerdown"));
    await nextTick();
    expect(clicked).toHaveBeenCalledOnce();
    expect(document.querySelector('[role="tooltip"]')).toBeNull();

    app.unmount();
  });
});
