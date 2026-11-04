import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, nextTick } from "vue";
import { mount } from "@vue/test-utils";
import { useAppShortcuts } from "./useAppShortcuts";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
import { installMemoryStorage } from "@/test/browserStorage";

beforeEach(() => {
  installMemoryStorage();
  setActivePinia(createPinia());
});
function setup() {
  const shortcuts = useShortcutStore();
  const settings = useSettingsStore();
  useWorkspaceStore().authenticated = true;
  const focus = vi.fn();
  const component = mount(
    defineComponent({
      setup() {
        useAppShortcuts(focus);
        return () => null;
      },
    }),
  );
  return { shortcuts, settings, focus, component };
}
function key(key: string, options: KeyboardEventInit = {}, target: EventTarget = window) {
  const event = new KeyboardEvent("keydown", { key, ctrlKey: true, bubbles: true, cancelable: true, ...options });
  target.dispatchEvent(event);
  return event;
}
describe("configurable application shortcuts", () => {
  it("applies a changed binding, disable and reset to the runtime handler", () => {
    const { shortcuts, settings, component } = setup();
    expect(shortcuts.set("sidebar", "Mod+Alt+B")).toBe("");
    key("b");
    expect(settings.sidebarCollapsed).toBe(false);
    key("b", { altKey: true });
    expect(settings.sidebarCollapsed).toBe(true);
    shortcuts.set("sidebar", "");
    key("b", { altKey: true });
    expect(settings.sidebarCollapsed).toBe(true);
    shortcuts.reset("sidebar");
    key("b");
    expect(settings.sidebarCollapsed).toBe(false);
    component.unmount();
    expect(key("b").defaultPrevented).toBe(false);
  });
  it("does not execute actions during recording, composition, repeated presses or dialogs", () => {
    const { focus, component } = setup();
    const input = document.createElement("input");
    input.dataset.shortcutRecorder = "";
    document.body.append(input);
    key("k", {}, input);
    key("k", { isComposing: true });
    key("k", { repeat: true });
    const dialog = document.createElement("div");
    dialog.dataset.slot = "dialog-content";
    dialog.dataset.state = "open";
    document.body.append(dialog);
    key("k");
    expect(focus).not.toHaveBeenCalled();
    input.remove();
    dialog.remove();
    component.unmount();
  });
  it("rejects collisions and keeps restored settings valid", async () => {
    const { shortcuts, component } = setup();
    expect(shortcuts.set("sidebar", "Mod+K")).not.toBe("");
    expect(shortcuts.bindings.sidebar).toBe("Mod+B");
    expect(shortcuts.set("sidebar", "Mod+R")).not.toBe("");
    expect(shortcuts.set("sidebar", "A")).not.toBe("");
    shortcuts.set("sidebar", "Alt+B");
    await nextTick();
    component.unmount();
    setActivePinia(createPinia());
    expect(useShortcutStore().bindings.sidebar).toBe("Alt+B");
  });
});
