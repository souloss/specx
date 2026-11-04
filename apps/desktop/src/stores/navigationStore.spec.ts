import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { useNavigationStore } from "./navigationStore";
import { useCreateQueueStore } from "./createQueueStore";
import { useWorkspaceStore } from "./workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { layoutPlacements } from "@/lib/sidebar/catalogNavigation";
import { installMemoryStorage } from "@/test/browserStorage";

vi.mock("@/lib/backend/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/backend/client")>()),
  specxApi: { createGroup: vi.fn(), createService: vi.fn(), updateGroup: vi.fn(), updateService: vi.fn(), deleteGroup: vi.fn(), deleteService: vi.fn(), reorderGroups: vi.fn() },
}));
beforeEach(() => {
  installMemoryStorage();
  vi.clearAllMocks();
  setActivePinia(createPinia());
});
function setup() {
  const workspace = useWorkspaceStore();
  workspace.catalog = catalogFixture();
  vi.spyOn(workspace, "refresh").mockResolvedValue(undefined);
  return { workspace, navigation: useNavigationStore() };
}
describe("catalog navigation commands", () => {
  it("coalesces Enter and blur while a name is being saved", async () => {
    const { workspace, navigation } = setup();
    let complete!: () => void;
    let createdId = "";
    vi.mocked(specxApi.createGroup).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = () => resolve({ id: createdId, name: "API", parentId: "platform", position: 1 });
        }),
    );
    navigation.beginCreate("group", "platform");
    navigation.naming!.name = "API";
    createdId = navigation.naming!.id;
    const first = navigation.commitName();
    await navigation.commitName();
    expect(specxApi.createGroup).toHaveBeenCalledTimes(1);
    expect(specxApi.createGroup).toHaveBeenCalledWith("workspace", expect.objectContaining({ name: "API", parentId: "platform" }), `create-group-${createdId}`);
    expect(navigation.naming).toBeNull();
    expect(navigation.busy).toBe(false);
    expect(useCreateQueueStore().current[0]?.status).toBe("saving");
    complete();
    await first;
    await useCreateQueueStore().retry(createdId);
    await nextTick();
    expect(navigation.naming).toBeNull();
    expect(workspace.refresh).not.toHaveBeenCalled();
    expect(workspace.catalog?.groups.find((item) => item.id === createdId)?.name).toBe("API");
  });
  it("moves a service using the current validator and reloads shared state", async () => {
    const { workspace, navigation } = setup();
    await navigation.drop("svc-b", "platform", "inside");
    expect(specxApi.updateService).toHaveBeenCalledWith("workspace", expect.objectContaining({ id: "svc-b", groupId: "platform", etag: '"v1"' }));
    expect(workspace.refresh).toHaveBeenCalledOnce();
    expect(layoutPlacements(navigation.layout).resources.get("svc-b")).toBe("platform");
  });
  it("keeps existing placement when a concurrent writer rejects the move", async () => {
    const { workspace, navigation } = setup();
    vi.mocked(specxApi.updateService).mockRejectedValueOnce(new Error("stale version"));
    await expect(navigation.drop("svc-b", "platform", "inside")).rejects.toThrow("stale version");
    expect(layoutPlacements(navigation.layout).resources.get("svc-b")).toBe("auth");
    expect(workspace.refresh).toHaveBeenCalledOnce();
    expect(navigation.busy).toBe(false);
  });
  it("locates an active service by clearing filters and expanding ancestors", async () => {
    const { workspace, navigation } = setup();
    navigation.collapseAll();
    navigation.query = "missing";
    navigation.tagId = "critical";
    workspace.selectService("svc-a");
    await nextTick();
    navigation.locateActive();
    expect(navigation.query).toBe("");
    expect(navigation.tagId).toBe("");
    expect([...navigation.expandedIds]).toEqual(["platform", "auth"]);
    expect(navigation.selectedId).toBe("svc-a");
  });
  it("keeps name editing recoverable after a failed save", async () => {
    const { navigation } = setup();
    navigation.beginRename({ id: "svc-a", type: "resource", label: "Service 2", parentId: "auth" });
    navigation.naming!.name = "Accounts";
    vi.mocked(specxApi.updateService).mockRejectedValueOnce(new Error("offline"));
    await expect(navigation.commitName()).rejects.toThrow("offline");
    expect(navigation.naming?.name).toBe("Accounts");
    expect(navigation.busy).toBe(false);
  });
});
