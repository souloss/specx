import { mount, flushPromises } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import OpenApiBulkEditDialog from "./OpenApiBulkEditDialog.vue";
let wrapper: ReturnType<typeof mount>;
afterEach(() => {
  wrapper?.unmount();
  document.body.innerHTML = "";
});
async function dialog() {
  wrapper = mount(OpenApiBulkEditDialog, { attachTo: document.body, props: { open: false, rows: [{ name: "q", value: "fox", enabled: true, type: "integer", description: "Count" }] } });
  await wrapper.setProps({ open: true });
  await flushPromises();
  return document.querySelector('[role="dialog"]')!;
}
function button(root: Element, text: string) {
  return [...root.querySelectorAll("button")].find((b) => b.textContent?.trim() === text)!;
}
describe("bulk editing modal", () => {
  it("keeps edits transactional and restores metadata across delimiter switches", async () => {
    const root = await dialog();
    button(root, "冒号模式").click();
    await flushPromises();
    const area = root.querySelector("textarea")!;
    area.value = "q:42";
    area.dispatchEvent(new Event("input", { bubbles: true }));
    button(root, "逗号模式").click();
    await flushPromises();
    expect(area.value).toContain("integer,Count");
    expect(area.value).toContain("42");
    button(root, "取消").click();
    await flushPromises();
    expect(wrapper.emitted("apply")).toBeUndefined();
  });
  it("rejects invalid CSV, preserves the draft and applies exactly once after correction", async () => {
    const root = await dialog();
    const area = root.querySelector("textarea")!;
    area.value = 'true,"unterminated';
    area.dispatchEvent(new Event("input", { bubbles: true }));
    button(root, "完成").click();
    await flushPromises();
    expect(root.querySelector('[role="alert"]')).not.toBeNull();
    expect(wrapper.emitted("apply")).toBeUndefined();
    area.value = 'true,q,"a,b",string,Query';
    area.dispatchEvent(new Event("input", { bubbles: true }));
    button(root, "完成").click();
    await flushPromises();
    expect(wrapper.emitted("apply")).toEqual([[[{ name: "q", value: "a,b", enabled: true, type: "string", description: "Query" }]]]);
  });
});
