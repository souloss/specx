import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import ResourceTree from "./ResourceTree.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import { useNavigationStore } from "@/stores/navigationStore";
import { catalogFixture } from "@/test/catalogFixture";
import { specxApi } from "@/lib/backend/client";
import { installMemoryStorage } from "@/test/browserStorage";
vi.mock("@/lib/backend/client", () => ({ specxApi: { updateService: vi.fn(), createGroup: vi.fn(async (_workspace, input) => ({ ...input, position: 0 })) } }));
let wrapper: VueWrapper;
beforeEach(() => {
  installMemoryStorage();
  vi.clearAllMocks();
  const pinia = createPinia();
  setActivePinia(pinia);
  const workspace = useWorkspaceStore();
  workspace.catalog = catalogFixture();
  vi.spyOn(workspace, "refresh").mockResolvedValue(undefined);
  wrapper = mount(ResourceTree, { attachTo: document.body, global: { plugins: [pinia] } });
});
afterEach(() => wrapper?.unmount());
describe("resource tree interactions", () => {
  it("opens the full service editor from the context menu", async () => {
    await wrapper.get('[data-tree-node="svc-a"]').trigger("contextmenu", { clientX: 60, clientY: 80 });
    const edit = Array.from(document.body.querySelectorAll("button")).find((button) => button.textContent?.includes("编辑服务"));
    expect(edit).toBeTruthy();
    edit!.click();
    expect(wrapper.emitted("editService")).toEqual([["svc-a"]]);
  });

  it("opens the migrated context menu and renames inline with Enter", async () => {
    await wrapper.get('[data-tree-node="svc-a"]').trigger("contextmenu", { clientX: 60, clientY: 80 });
    const rename = Array.from(document.body.querySelectorAll("button")).find((button) => button.textContent?.includes("重命名"));
    expect(rename).toBeTruthy();
    rename!.click();
    await nextTick();
    const input = wrapper.get('input[aria-label="名称"]');
    await input.setValue("Accounts");
    await input.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(specxApi.updateService).toHaveBeenCalledWith("workspace", expect.objectContaining({ id: "svc-a", name: "Accounts" }));
    expect(wrapper.find("input").exists()).toBe(false);
  });
  it("cancels an inline creation with Escape without saving", async () => {
    const navigation = useNavigationStore();
    navigation.beginCreate("group", "platform");
    await nextTick();
    const input = wrapper.get('input[aria-label="名称"]');
    await input.setValue("draft");
    await input.trigger("keydown", { key: "Escape" });
    await flushPromises();
    expect(specxApi.createGroup).not.toHaveBeenCalled();
    expect(navigation.naming).toBeNull();
  });
  it("saves a new group on blur and exposes accessible tree expansion", async () => {
    const navigation = useNavigationStore();
    navigation.beginCreate("group", "platform");
    await nextTick();
    const input = wrapper.get('input[aria-label="名称"]');
    await input.setValue("API");
    await input.trigger("blur");
    await flushPromises();
    expect(specxApi.createGroup).toHaveBeenCalledOnce();
    await wrapper.get('[data-tree-node="platform"]').trigger("click");
    expect(wrapper.get('[data-tree-node="platform"]').attributes("aria-expanded")).toBe("false");
    expect(wrapper.find('[data-tree-node="svc-a"]').exists()).toBe(false);
  });
});
