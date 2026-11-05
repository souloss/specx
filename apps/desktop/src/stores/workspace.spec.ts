import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useWorkspaceStore } from "./workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { installMemoryStorage } from "@/test/browserStorage";
import type { Catalog } from "@/types/domain";

vi.mock("@/lib/backend/client", () => ({ specxApi: { migrationRetry: vi.fn(), authCheck: vi.fn(), listWorkspaces: vi.fn(), loadCatalog: vi.fn() } }));
beforeEach(() => {
  installMemoryStorage();
  vi.resetAllMocks();
  setActivePinia(createPinia());
  vi.mocked(specxApi.migrationRetry).mockResolvedValue({ ready: true });
  vi.mocked(specxApi.listWorkspaces).mockResolvedValue({ items: [] });
});
describe("catalog loading", () => {
  it("uses one workspace list and preserves creates confirmed during an earlier refresh", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    let complete!: (catalog: Catalog) => void;
    vi.mocked(specxApi.loadCatalog).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const loading = workspace.refresh();
    await vi.waitFor(() => expect(specxApi.loadCatalog).toHaveBeenCalledOnce());
    workspace.catalog = { ...workspace.catalog!, groups: [...workspace.catalog!.groups, { id: "created", name: "Created", parentId: null }] };
    complete(catalogFixture());
    await loading;
    expect(specxApi.listWorkspaces).toHaveBeenCalledOnce();
    expect(specxApi.loadCatalog).toHaveBeenCalledWith("workspace", []);
    expect(workspace.catalog.groups.some((item) => item.id === "created")).toBe(true);
  });
  it("ignores an older workspace load that finishes after the latest selection", async () => {
    const workspace = useWorkspaceStore();
    let complete!: (catalog: Catalog) => void;
    vi.mocked(specxApi.loadCatalog).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const first = workspace.load("workspace");
    await vi.waitFor(() => expect(specxApi.loadCatalog).toHaveBeenCalledOnce());
    vi.mocked(specxApi.loadCatalog).mockResolvedValueOnce({ ...catalogFixture(), workspace: { id: "latest", name: "Latest", tenantName: "tenant" } });
    await workspace.load("latest");
    complete(catalogFixture());
    await first;
    expect(workspace.catalog?.workspace.id).toBe("latest");
    expect(workspace.loading).toBe(false);
  });

  it("keeps the app out of the business startup path while desktop migration is blocked", async () => {
    vi.mocked(specxApi.migrationRetry).mockResolvedValue({ ready: false });
    const workspace = useWorkspaceStore();

    await workspace.initialize();

    expect(workspace.migrationReady).toBe(false);
    expect(workspace.error).toBe("specX 存储迁移尚未完成");
    expect(specxApi.authCheck).not.toHaveBeenCalled();
  });
});
