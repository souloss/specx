import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import OpenApiDesignAddResponseDialog from "./OpenApiDesignAddResponseDialog.vue";

let wrapper: ReturnType<typeof mount>;

afterEach(() => {
  wrapper?.unmount();
  document.body.innerHTML = "";
});

function changeSelect(name: string, value: string) {
  const select = document.querySelector<HTMLSelectElement>(`select[name="${name}"]`)!;
  select.value = value;
  select.dispatchEvent(new Event("change", { bubbles: true }));
}

describe("add response dialog", () => {
  it("chooses a predefined status, supports no-body statuses, and adds a named media response", async () => {
    wrapper = mount(OpenApiDesignAddResponseDialog, { attachTo: document.body, props: { open: false, usedStatuses: ["200"] } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.querySelector<HTMLInputElement>('input[name="openapi-new-response-name"]')!.value).toBe("Created");
    expect(dialog.querySelector('button[role="combobox"][aria-label="响应状态码"]')).not.toBeNull();
    const statusOptions = [...dialog.querySelectorAll<HTMLSelectElement>('select[name="openapi-new-response-status"] option')].map((option) => option.value);
    expect(statusOptions).toContain("2XX");
    expect(statusOptions).toContain("511");
    changeSelect("openapi-new-response-status", "204");
    await flushPromises();
    expect(dialog.textContent).toContain("此状态码不返回响应体");
    expect(dialog.querySelector('select[name="openapi-new-response-media-type"]')).toBeNull();

    changeSelect("openapi-new-response-status", "422");
    await flushPromises();
    expect(dialog.querySelector('button[role="combobox"][aria-label="媒体类型"]')).not.toBeNull();
    const name = dialog.querySelector<HTMLInputElement>('input[name="openapi-new-response-name"]')!;
    name.value = "Validation failed";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    changeSelect("openapi-new-response-media-type", "text/plain");
    dialog.querySelector<HTMLButtonElement>('button[form="openapi-add-response-form"]')!.click();
    await flushPromises();

    expect(wrapper.emitted("add")).toEqual([[{ status: "422", description: "Validation failed", mediaType: "text/plain" }]]);
  });
});
