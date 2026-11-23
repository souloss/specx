import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useWorkspaceStore } from "./workspace";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "@/test/catalogFixture";
import { installMemoryStorage } from "@/test/browserStorage";
import type { Catalog } from "@/types/domain";
import { ApiError } from "@/lib/backend/types";

vi.mock("@/lib/backend/client", () => ({
  specxApi: {
    migrationRetry: vi.fn(),
    authCheck: vi.fn(),
    listWorkspaces: vi.fn(),
    loadCatalog: vi.fn(),
    updateSpecification: vi.fn(),
    updateService: vi.fn(),
    listSpecificationRevisions: vi.fn(),
    restoreSpecificationRevision: vi.fn(),
    deleteSpecification: vi.fn(),
    syncSpecification: vi.fn(),
    setSpecificationCodeBinding: vi.fn(),
    deleteSpecificationCodeBinding: vi.fn(),
  },
  toService: (service: Record<string, unknown>) => ({ ...service, groupId: service.groupId ?? "ungrouped", createdAt: service.updatedAt }),
}));
beforeEach(() => {
  installMemoryStorage();
  vi.resetAllMocks();
  setActivePinia(createPinia());
  vi.mocked(specxApi.migrationRetry).mockResolvedValue({ ready: true });
  vi.mocked(specxApi.listWorkspaces).mockResolvedValue({ items: [] });
});
describe("catalog loading", () => {
  it("updates service metadata through the ETag guarded client and keeps the open tab label current", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.openOverview("svc-a");
    vi.mocked(specxApi.updateService).mockResolvedValue({ ...workspace.catalog.services[1], name: "Accounts API", description: "账户服务", tagIds: ["critical"], updatedAt: "2026-11-08", groupId: "auth", archived: false, etag: '"v3"' });

    await workspace.updateService({ ...workspace.catalog.services[1], name: "Accounts API", description: "账户服务", tagIds: ["critical"] });

    expect(specxApi.updateService).toHaveBeenCalledWith("workspace", expect.objectContaining({ id: "svc-a", name: "Accounts API", description: "账户服务", tagIds: ["critical"] }));
    expect(workspace.catalog.services.find((service) => service.id === "svc-a")).toMatchObject({ name: "Accounts API", description: "账户服务", tagIds: ["critical"] });
    expect(workspace.tabs.find((tab) => tab.serviceId === "svc-a")?.title).toBe("Accounts API");
  });

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

  it("preserves an OpenAPI operation view when saving its source document", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    specification.content = JSON.stringify({ openapi: "3.0.3", info: { title: "Accounts", version: "1" }, paths: {} });
    workspace.openSpecification("spec", "GET:/accounts", "List accounts");
    workspace.beginSpecificationEdit("spec");
    workspace.updateSpecificationDraft("spec", { title: "Accounts API", content: JSON.stringify({ openapi: "3.0.3", info: { title: "Accounts", version: "1" }, paths: {} }, null, 2) });
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({
      ...specification,
      title: "Accounts API",
      version: 2,
      etag: '"spec-v2"',
    });

    await expect(workspace.saveSpecification("spec")).resolves.toBe(true);

    expect(workspace.catalog?.specifications[0]).toMatchObject({ kind: "openapi", title: "Accounts API", version: 2 });
    expect(workspace.tabs[0]).toMatchObject({ operationKey: "GET:/accounts", title: "List accounts" });
    vi.mocked(specxApi.loadCatalog).mockResolvedValue(JSON.parse(JSON.stringify(workspace.catalog)) as Catalog);
    await workspace.refresh();
    expect(workspace.tabs[0]).toMatchObject({ operationKey: "GET:/accounts", title: "List accounts" });
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

  it("saves an operation with the current ETag while preserving an independent full-source draft", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    workspace.updateSpecificationDraft("spec", { title: "Unsaved source title", content: "Unsaved full-source content" });
    specification.etag = '"spec-v2"';
    specification.version = 2;
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({ ...specification, content: "Saved operation content", etag: '"spec-v3"', version: 3 });
    await expect(workspace.saveOpenApiOperation("spec", "Saved operation content")).resolves.toBe(true);
    expect(specxApi.updateSpecification).toHaveBeenCalledWith("workspace", "spec", { title: specification.title, content: "Saved operation content" }, '"spec-v2"');
    expect(workspace.catalog.specifications[0]).toMatchObject({ content: "Saved operation content", version: 3, kind: "openapi" });
    expect(workspace.specificationDraft("spec")).toMatchObject({ title: "Unsaved source title", content: "Unsaved full-source content", baseEtag: '"spec-v1"' });
  });

  it("refreshes a concurrent operation change without overwriting its source draft", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.catalog.specifications[0]!.kind = "openapi";
    workspace.updateSpecificationDraft("spec", { title: "Local source draft", content: "local" });
    vi.mocked(specxApi.updateSpecification).mockRejectedValue(new ApiError("Conflict", 412));
    const remote = catalogFixture();
    remote.specifications[0] = { ...remote.specifications[0]!, kind: "openapi", content: "remote", version: 2, etag: '"spec-v2"' };
    vi.mocked(specxApi.loadCatalog).mockResolvedValue(remote);
    await expect(workspace.saveOpenApiOperation("spec", "operation draft")).rejects.toThrow("设计草稿仍保留");
    expect(workspace.catalog.specifications[0]).toMatchObject({ content: "remote", version: 2 });
    expect(workspace.specificationDraft("spec")).toMatchObject({ title: "Local source draft", content: "local", baseEtag: '"spec-v1"' });
  });

  it("does not put a late operation save into another workspace", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.catalog.specifications[0]!.kind = "openapi";
    const saved = { ...workspace.catalog.specifications[0]!, content: "saved in original workspace", version: 2 };
    let complete!: (value: typeof saved) => void;
    vi.mocked(specxApi.updateSpecification).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const saving = workspace.saveOpenApiOperation("spec", saved.content);
    workspace.catalog = { ...catalogFixture(), workspace: { id: "another-workspace", name: "Another", tenantName: "Tenant" } };
    complete(saved);
    await expect(saving).resolves.toBe(true);
    expect(workspace.catalog.specifications[0]!.content).not.toBe(saved.content);
  });

  it("opens a newly created API while keeping its specification root tab", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    workspace.openSpecification("spec");
    const content = JSON.stringify({ openapi: "3.0.3", info: { title: "API", version: "1" }, paths: { "/orders/{id}": { get: { responses: { "200": { description: "OK" } } } } } });
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({ ...specification, content, version: 2, etag: '"spec-v2"' });
    await workspace.saveOpenApiOperation("spec", content, { key: "GET:/orders/{id}", title: "GET /orders/{id}" });
    expect(workspace.tabs).toHaveLength(2);
    expect(workspace.tabs[0]).toMatchObject({ id: "specification-spec", operationKey: undefined });
    expect(workspace.activeTab).toMatchObject({ operationKey: "GET:/orders/{id}", title: "GET /orders/{id}" });
  });

  it("persists separate model targets and retargets only the renamed model tab", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    specification.content = JSON.stringify({ openapi: "3.1.0", components: { schemas: { Pet: { type: "object" }, Other: { type: "string" } } } });
    workspace.openSpecification("spec");
    workspace.openOpenApiModel("spec");
    workspace.openOpenApiModel("spec", "#/components/schemas/Pet");
    workspace.openOpenApiModel("spec", "#/components/schemas/Pet");
    expect(workspace.tabs).toHaveLength(3);
    expect(workspace.activeTab).toMatchObject({ modelRef: "#/components/schemas/Pet", title: "Pet" });
    const next = JSON.stringify({ openapi: "3.1.0", components: { schemas: { PetRecord: { type: "object" }, Other: { type: "string" } } } });
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({ ...specification, content: next, version: 2, etag: '"spec-v2"' });
    await workspace.saveOpenApiDocument("spec", next, { kind: "model", previousReference: "#/components/schemas/Pet", reference: "#/components/schemas/PetRecord", title: "PetRecord" });
    expect(specxApi.updateSpecification).toHaveBeenCalledWith("workspace", "spec", { title: specification.title, content: next }, specification.etag);
    expect(workspace.activeTab).toMatchObject({ modelRef: "#/components/schemas/PetRecord", title: "PetRecord" });
    expect(workspace.tabs[1]).toMatchObject({ modelsDirectory: true, title: "数据模型" });
    vi.mocked(specxApi.loadCatalog).mockResolvedValue(JSON.parse(JSON.stringify(workspace.catalog)) as Catalog);
    await workspace.refresh();
    expect(workspace.activeTab).toMatchObject({ modelRef: "#/components/schemas/PetRecord", title: "PetRecord" });
    workspace.openOpenApiModel("spec", "#/components/schemas/Missing");
    expect(workspace.activeTab?.modelRef).toBe("#/components/schemas/PetRecord");
  });
  it("retargets the active operation tab after an address change and removes a stale duplicate tab", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    workspace.openSpecification("spec", "POST:/orders/{orderId}", "Stale destination tab");
    workspace.openSpecification("spec", "GET:/orders/{id}", "GET /orders/{id}");
    const content = JSON.stringify({ openapi: "3.0.3", info: { title: "API", version: "1" }, paths: { "/orders/{orderId}": { post: { responses: { "200": { description: "OK" } } } } } });
    vi.mocked(specxApi.updateSpecification).mockResolvedValue({ ...specification, content, version: 2, etag: '"spec-v2"' });
    await workspace.saveOpenApiOperation("spec", content, { previousKey: "GET:/orders/{id}", key: "POST:/orders/{orderId}", title: "POST /orders/{orderId}" });
    expect(workspace.tabs).toHaveLength(1);
    expect(workspace.activeTab).toMatchObject({ id: "specification-spec-POST%3A%2Forders%2F%7BorderId%7D", operationKey: "POST:/orders/{orderId}", title: "POST /orders/{orderId}" });
  });

  it("keeps the user's current page when an API creation finishes after they navigate away", async () => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    const specification = workspace.catalog.specifications[0]!;
    specification.kind = "openapi";
    workspace.openSpecification("spec");
    const content = JSON.stringify({ openapi: "3.0.3", info: { title: "API", version: "1" }, paths: { "/orders": { get: { responses: { "200": { description: "OK" } } } } } });
    const saved = { ...specification, content, version: 2, etag: '"spec-v2"' };
    let complete!: (value: typeof saved) => void;
    vi.mocked(specxApi.updateSpecification).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const saving = workspace.saveOpenApiOperation("spec", content, { key: "GET:/orders", title: "GET /orders" });
    workspace.openOverview("svc-a");
    complete(saved);
    await saving;
    expect(workspace.activeTab).toMatchObject({ kind: "overview", serviceId: "svc-a" });
    expect(workspace.catalog.specifications[0]!.content).toBe(content);
    expect(workspace.tabs.some((tab) => tab.operationKey === "GET:/orders")).toBe(false);
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

  it.each(["markdown", "openapi"] as const)("restores a %s revision through the current ETag and preserves its provider", async (kind) => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.catalog.specifications[0]!.kind = kind;
    const content = kind === "markdown" ? "# 恢复内容" : JSON.stringify({ openapi: "3.1.0", info: { title: "Restored", version: "1" }, paths: {} });
    vi.mocked(specxApi.restoreSpecificationRevision).mockResolvedValue({
      ...workspace.catalog.specifications[0],
      content,
      version: 3,
      etag: '"spec-v3"',
    });
    vi.mocked(specxApi.listSpecificationRevisions).mockResolvedValue({ items: [], total: 0, limit: 50, offset: 0 });

    await expect(workspace.restoreSpecificationRevision("spec", 1)).resolves.toBe(true);

    expect(specxApi.restoreSpecificationRevision).toHaveBeenCalledWith("workspace", "spec", 1, '"spec-v1"');
    expect(workspace.catalog.specifications[0]).toMatchObject({ content, kind, version: 3 });
    expect(specxApi.listSpecificationRevisions).toHaveBeenCalledWith("workspace", "spec");
  });

  it.each(["markdown", "openapi"] as const)("preserves the %s provider when synchronizing a managed specification", async (kind) => {
    const workspace = useWorkspaceStore();
    workspace.catalog = catalogFixture();
    workspace.catalog.specifications[0]!.kind = kind;
    const content = kind === "markdown" ? "# Synced" : JSON.stringify({ openapi: "3.1.0", info: { title: "Synced", version: "1" }, paths: {} });
    vi.mocked(specxApi.syncSpecification).mockResolvedValue({
      specification: { ...workspace.catalog.specifications[0], content, contentHash: "sha256:synced", version: 2, etag: '"spec-v2"' },
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
    expect(workspace.catalog.specifications[0]).toMatchObject({ content, kind, version: 2, contentHash: "sha256:synced" });
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
