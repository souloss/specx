import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import ShortcutSettings from "./ShortcutSettings.vue";
import { useShortcutStore } from "@/stores/shortcutStore";
import { installMemoryStorage } from "@/test/browserStorage";
import { setLocale } from "@/i18n";

let wrapper: VueWrapper;
beforeEach(() => {
  installMemoryStorage();
  setLocale("zh-CN");
  const pinia = createPinia();
  setActivePinia(pinia);
  wrapper = mount(ShortcutSettings, { attachTo: document.body, global: { plugins: [pinia] } });
});
afterEach(() => wrapper.unmount());
describe("shortcut settings interactions", () => {
  it("records a binding and supports conflict feedback, cancellation, disabling and reset", async () => {
    const shortcuts = useShortcutStore();
    const input = wrapper.get<HTMLInputElement>("#shortcut-sidebar");
    await input.trigger("click");
    await input.trigger("keydown", { key: "b", ctrlKey: true, altKey: true });
    expect(shortcuts.bindings.sidebar).toBe("Mod+Alt+B");
    await input.trigger("click");
    await input.trigger("keydown", { key: "k", ctrlKey: true });
    expect(wrapper.get('[role="alert"]').text()).toContain("搜索服务或分组");
    expect(shortcuts.bindings.sidebar).toBe("Mod+Alt+B");
    await input.trigger("keydown", { key: "Escape" });
    await input.trigger("click");
    await input.trigger("keydown", { key: "Backspace" });
    expect(shortcuts.bindings.sidebar).toBe("");
    await wrapper.get('button[aria-label="重置展开 / 收起侧边栏快捷键"]').trigger("click");
    expect(shortcuts.bindings.sidebar).toBe("Mod+B");
    await input.trigger("focus");
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    input.element.dispatchEvent(tab);
    await nextTick();
    expect(tab.defaultPrevented).toBe(false);
    expect(input.element.value).toBe("Ctrl + B");
  });
  it("searches current combinations and expands matching groups without losing collapsed state", async () => {
    const group = wrapper.get('button[aria-controls="shortcut-group-global"]');
    await group.trigger("click");
    expect(group.attributes("aria-expanded")).toBe("false");
    await wrapper.get('input[aria-label="搜索操作、分类或快捷键"]').setValue("ctrl + k");
    expect(wrapper.get('button[aria-controls="shortcut-group-global"]').attributes("aria-expanded")).toBe("true");
    expect(wrapper.findAll("[data-shortcut-recorder]")).toHaveLength(1);
    expect(wrapper.find("#shortcut-search").exists()).toBe(true);
    await wrapper.get('input[aria-label="搜索操作、分类或快捷键"]').setValue("");
    expect(wrapper.get('button[aria-controls="shortcut-group-global"]').attributes("aria-expanded")).toBe("false");
  });
});
