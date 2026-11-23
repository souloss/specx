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
  it("expands a service from its disclosure control without changing its selected view", async () => {
    const navigation = useNavigationStore();
    const service = catalogFixture().services.find((item) => item.id === "svc-a")!;
    const catalog = useWorkspaceStore().catalog!;
    catalog.specifications.push({
      id: "spec-a",
      serviceId: service.id,
      title: "Accounts API",
      kind: "openapi",
      content: JSON.stringify({ openapi: "3.0.3", info: { title: "Accounts", version: "1" }, paths: { "/accounts": { get: { tags: ["Accounts"] } } } }),
      version: 1,
      contentHash: "sha256:test",
      updatedAt: "2026-11-08",
      etag: '"spec-v1"',
    });
    await nextTick();
    const disclosure = wrapper.get('[data-tree-node="svc-a"] button[aria-expanded="false"]');
    await disclosure.trigger("click");
    await nextTick();
    expect(wrapper.get('[data-tree-node="svc-a"]').attributes("aria-expanded")).toBe("true");
    const provider = wrapper.get('[data-tree-node="specification-provider:svc-a:openapi"]');
    expect(provider.text()).toContain("OpenAPI");
    await provider.get('button[aria-expanded="false"]').trigger("click");
    await nextTick();
    expect(wrapper.get('[data-tree-node="specification:spec-a"]').text()).toContain("Accounts API");
    expect(navigation.selectedId).not.toBe("specification:spec-a");
  });
  it("offers specification actions for document nodes instead of opening a service", async () => {
    const serviceDisclosure = wrapper.get('[data-tree-node="svc-a"] button[aria-expanded="false"]');
    await serviceDisclosure.trigger("click");
    await nextTick();
    const provider = wrapper.get('[data-tree-node="specification-provider:svc-a:markdown"]');
    await provider.get('button[aria-expanded="false"]').trigger("click");
    await nextTick();
    await wrapper.get('[data-tree-node="specification:spec"]').trigger("contextmenu", { clientX: 60, clientY: 80 });
    const menuButtons = Array.from(document.body.querySelectorAll("button"));
    expect(menuButtons.some((button) => button.textContent?.includes("打开规范"))).toBe(true);
    expect(menuButtons.some((button) => button.textContent?.includes("打开服务"))).toBe(false);
  });
});
