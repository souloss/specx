import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useWorkspaceStore } from "./workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { installMemoryStorage } from "@/test/browserStorage";
import type { Catalog } from "@/types/domain";

vi.mock("@/lib/backend/client", () => ({
  specxApi: {
    migrationRetry: vi.fn(),
    authCheck: vi.fn(),
    listWorkspaces: vi.fn(),
    loadCatalog: vi.fn(),
    updateSpecification: vi.fn(),
    listSpecificationRevisions: vi.fn(),
    restoreSpecificationRevision: vi.fn(),
    deleteSpecification: vi.fn(),
    syncSpecification: vi.fn(),
    setSpecificationCodeBinding: vi.fn(),
    deleteSpecificationCodeBinding: vi.fn(),
  },
}));
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

  it("keeps a local draft until an ETag guarded update succeeds", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.openSpecification("spec");
    workspace.beginSpecificationEdit("spec");
    workspace.updateSpecificationDraft("spec", { title: "访问约定 v2", content: "# 新约定" });
    expect(workspace.hasUnsavedSpecification("spec")).toBe(true);
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({
      ...workspace.catalog.specifications[0],
      title: "访问约定 v2",
      content: "# 新约定",
      version: 2,
      etag: '"spec-v2"',
    });

    await expect(workspace.saveSpecification("spec")).resolves.toBe(true);

    expect(specxApi.updateSpecification).toHaveBeenCalledWith("workspace", "spec", { title: "访问约定 v2", content: "# 新约定" }, '"spec-v1"');
    expect(workspace.catalog.specifications[0]).toMatchObject({ title: "访问约定 v2", version: 2, etag: '"spec-v2"' });
    expect(workspace.tabs[0]).toMatchObject({ title: "访问约定 v2" });
    expect(workspace.specificationDraft("spec")).toBeUndefined();
  });

  it("retains the draft and records the remote representation on an ETag conflict", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.beginSpecificationEdit("spec");
    workspace.updateSpecificationDraft("spec", { title: "本地版本", content: "# 本地" });
    vi.mocked(specxApi.updateSpecification).mockRejectedValue({ status: 412 });
    vi.mocked(specxApi.loadCatalog).mockResolvedValue({
      ...catalogFixture(),
      specifications: [{ ...catalogFixture().specifications[0], title: "远端版本", content: "# 远端", version: 2, etag: '"spec-v2"' }],
    });

    await expect(workspace.saveSpecification("spec")).resolves.toBe(false);

    expect(workspace.specificationDraft("spec")).toMatchObject({ status: "conflict", title: "本地版本", remote: { title: "远端版本", version: 2 } });
  });

  it("requires an explicit decision before closing a tab with unsaved content", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.openSpecification("spec");
    workspace.beginSpecificationEdit("spec");
    workspace.updateSpecificationDraft("spec", { title: "草稿", content: "# 草稿" });

    expect(workspace.closeTab("specification-spec")).toBe(false);
    expect(workspace.pendingCloseTabId).toBe("specification-spec");
    await workspace.resolvePendingClose("discard");
    expect(workspace.tabs).toHaveLength(0);
    expect(workspace.specificationDraft("spec")).toBeUndefined();
  });

  it("restores a revision through the current ETag and refreshes history", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    vi.mocked(specxApi.restoreSpecificationRevision).mockResolvedValue({
      ...workspace.catalog.specifications[0],
      content: "# 恢复内容",
      version: 3,
      etag: '"spec-v3"',
    });
    vi.mocked(specxApi.listSpecificationRevisions).mockResolvedValue({ items: [], total: 0, limit: 50, offset: 0 });

    await expect(workspace.restoreSpecificationRevision("spec", 1)).resolves.toBe(true);

    expect(specxApi.restoreSpecificationRevision).toHaveBeenCalledWith("workspace", "spec", 1, '"spec-v1"');
    expect(workspace.catalog.specifications[0]).toMatchObject({ content: "# 恢复内容", version: 3 });
    expect(specxApi.listSpecificationRevisions).toHaveBeenCalledWith("workspace", "spec");
  });

  it("updates the catalog and records the result after synchronizing a managed specification", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    vi.mocked(specxApi.syncSpecification).mockResolvedValue({
      specification: { ...workspace.catalog.specifications[0], content: "# Synced", contentHash: "sha256:synced", version: 2, etag: '"spec-v2"' },
      snapshotId: "snapshot-1",
      manifestHash: "sha256:manifest",
      resolvedCommit: "a".repeat(40),
      contentHash: "sha256:synced",
      changed: true,
      syncedAt: "2026-11-07T00:00:00Z",
      providerVersion: "git-cli",
    });

    await expect(workspace.syncSpecification("spec")).resolves.toMatchObject({ changed: true, providerVersion: "git-cli" });

    expect(specxApi.syncSpecification).toHaveBeenCalledWith("workspace", "spec");
    expect(workspace.catalog.specifications[0]).toMatchObject({ content: "# Synced", version: 2, contentHash: "sha256:synced" });
    expect(workspace.specificationSyncFor("spec")).toMatchObject({ changed: true, resolvedCommit: "a".repeat(40) });
  });

  it("clears stale sync evidence when the managed source changes", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    vi.mocked(specxApi.syncSpecification).mockResolvedValue({
      specification: workspace.catalog.specifications[0],
      snapshotId: "snapshot-1",
      manifestHash: "sha256:manifest",
      resolvedCommit: "a".repeat(40),
      contentHash: workspace.catalog.specifications[0].contentHash,
      changed: false,
      syncedAt: "2026-11-07T00:00:00Z",
      providerVersion: "git-cli",
    });
    vi.mocked(specxApi.setSpecificationCodeBinding).mockResolvedValue({
      id: "binding-1",
      specificationId: "spec",
      repository: { id: "repo-1", provider: "github", displayName: "acme/api", canonicalUrl: "https://github.com/acme/api" },
      path: "docs/new.md",
      refKind: "branch",
      refName: "main",
      createdAt: "2026-11-07T00:00:00Z",
      updatedAt: "2026-11-07T00:00:00Z",
      etag: '"binding-v1"',
    });

    await workspace.syncSpecification("spec");
    expect(workspace.specificationSyncFor("spec")).not.toBeNull();
    await workspace.setSpecificationCodeBinding("spec", "branch", "main", "docs/new.md");

    expect(workspace.specificationSyncFor("spec")).toBeNull();
  });
});
