import { afterEach, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { nextTick } from "vue";
import SettingsPage from "./SettingsPage.vue";
import { setLocale } from "@/i18n";

afterEach(() => setLocale("zh-CN"));

it("updates settings labels when the locale changes", async () => {
  setLocale("zh-CN");
  const wrapper = mount(SettingsPage, { global: { plugins: [createPinia()] } });

  expect(wrapper.text()).toContain("外观与主题");
  expect(wrapper.text()).toContain("显示语言");

  setLocale("en");
  await nextTick();

  expect(wrapper.text()).toContain("Appearance");
  expect(wrapper.text()).toContain("Display language");
  expect(wrapper.get("#ui-font-family").text()).toBe("Geist (default)");
  wrapper.unmount();
});
