import { flushPromises, mount } from "@vue/test-utils";
import { nextTick, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import ServicePanel from "./ServicePanel.vue";
import { catalogFixture } from "@/test/catalogFixture";

const mockWorkspace = vi.hoisted(() => ({
  workspaces: [],
  specificationCodeBindingFor: vi.fn(),
  specificationSyncFor: vi.fn(),
  loadSpecificationCodeBinding: vi.fn(),
  syncSpecification: vi.fn(),
  deleteSpecification: vi.fn(),
  openOverview: vi.fn(),
  openApiProviderFor: vi.fn(() => null),
  loadOpenApiProvider: vi.fn(async () => null),
  serviceRepositoryFor: vi.fn(() => null),
  loadServiceRepository: vi.fn(async () => null),
}));

vi.mock("@/stores/workspace", () => ({
  useWorkspaceStore: () => mockWorkspace,
}));

function managedPanel() {
  const catalog = catalogFixture();
  const binding = {
    id: "binding",
    specificationId: "spec",
    repository: { id: "repo", provider: "github", displayName: "specx/specx", canonicalUrl: "https://github.com/specx/specx" },
    path: "docs/api.md",
    refKind: "branch" as const,
    refName: "main",
    createdAt: "2026-11-07T00:00:00Z",
    updatedAt: "2026-11-07T00:00:00Z",
    etag: '"binding-v1"',
  };
  mockWorkspace.specificationCodeBindingFor.mockReturnValue(binding);
  let syncResult: unknown = null;
  const syncRevision = ref(0);
  mockWorkspace.specificationSyncFor.mockImplementation(() => {
    void syncRevision.value;
    return syncResult;
  });
  mockWorkspace.loadSpecificationCodeBinding.mockResolvedValue(binding);
  const tab = { id: "specification-spec", kind: "specification" as const, serviceId: "svc-a", specificationId: "spec", title: "访问约定" };
  return {
    catalog,
    tab,
    publishSyncResult: (value: unknown) => {
      syncResult = value;
      syncRevision.value += 1;
    },
  };
}

describe("ServicePanel repository-managed specification", () => {
  it("offers a guarded sync action and shows the completed result", async () => {
    const { catalog, tab, publishSyncResult } = managedPanel();
    let complete!: (value: unknown) => void;
    const result = {
      specification: { ...catalog.specifications[0], content: "# Synced", contentHash: "sha256:synced", version: 2, etag: '"spec-v2"' },
      resolvedCommit: "a".repeat(40),
      contentHash: "sha256:synced",
      changed: true,
      syncedAt: "2026-11-07T00:00:00Z",
      providerVersion: "git-cli",
      snapshotId: "snapshot-1",
      manifestHash: "sha256:manifest",
    };
    mockWorkspace.syncSpecification.mockImplementationOnce(() =>
      new Promise((resolve) => {
        complete = resolve;
      }).then(() => {
        publishSyncResult(result);
        catalog.specifications[0] = result.specification;
        return result;
      }),
    );

    const wrapper = mount(ServicePanel, {
      props: { catalog, serviceId: "svc-a", tab },
      global: {
        stubs: {
          MarkdownPreview: { template: "<div />" },
          SpecificationEditor: { template: "<div />" },
          SpecificationHistory: { template: "<div />" },
          SpecificationCodeBindingDialog: { template: "<div />" },
          CodeBindingDialog: { template: "<div />" },
          Dialog: { template: "<div><slot /></div>" },
          DialogContent: { template: "<div><slot /></div>" },
          DialogFooter: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<div><slot /></div>" },
        },
      },
    });
    await flushPromises();

    const syncButton = () => wrapper.findAll("button").find((button) => button.text().includes("立即同步") || button.text().includes("正在同步"))!;
    expect(syncButton().exists()).toBe(true);
    expect(wrapper.text()).toContain("由仓库托管");
    expect(wrapper.text()).not.toContain("编辑规范");

    await syncButton().trigger("click");
    await nextTick();
    expect(syncButton().attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("正在同步…");

    complete(result);
    await flushPromises();
    expect(mockWorkspace.syncSpecification).toHaveBeenCalledWith("spec");
    expect(wrapper.text()).toContain("已从 aaaaaaa 更新");
    expect(wrapper.text()).toContain("最近同步于");
    expect(wrapper.text()).toContain("清单 sha256:manifes");
    wrapper.unmount();
  });

  it("uses the shared destructive button variant for specification deletion", async () => {
    const catalog = catalogFixture();
    mockWorkspace.specificationCodeBindingFor.mockReturnValue(null);
    mockWorkspace.specificationSyncFor.mockReturnValue(null);
    mockWorkspace.loadSpecificationCodeBinding.mockResolvedValue(null);
    const wrapper = mount(ServicePanel, {
      props: {
        catalog,
        serviceId: "svc-a",
        tab: { id: "specification-spec", kind: "specification", serviceId: "svc-a", specificationId: "spec", title: "访问约定" },
      },
      global: {
        stubs: {
          MarkdownPreview: { template: "<div />" },
          SpecificationEditor: { template: "<div />" },
          SpecificationHistory: { template: "<div />" },
          SpecificationCodeBindingDialog: { template: "<div />" },
          CodeBindingDialog: { template: "<div />" },
          Dialog: { template: "<div><slot /></div>" },
          DialogContent: { template: "<div><slot /></div>" },
          DialogFooter: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<div><slot /></div>" },
        },
      },
    });
    await flushPromises();

    const deleteButton = wrapper.findAll("button").find((button) => button.text().includes("删除规范"));
    expect(deleteButton?.attributes("data-variant")).toBe("destructive");
    wrapper.unmount();
  });

  it("reloads source and environment metadata after a catalog refresh while guarding edits", async () => {
    const catalog = catalogFixture();
    catalog.specifications[0] = { ...catalog.specifications[0]!, kind: "openapi", content: JSON.stringify({ openapi: "3.1.0", paths: {} }) };
    const binding = ref<null | { path: string }>(null);
    mockWorkspace.specificationCodeBindingFor.mockImplementation(() => binding.value);
    mockWorkspace.specificationSyncFor.mockReturnValue(null);
    mockWorkspace.loadSpecificationCodeBinding.mockResolvedValue(null);
    const wrapper = mount(ServicePanel, {
      props: { catalog, serviceId: "svc-a", tab: { id: "specification-spec", kind: "specification", serviceId: "svc-a", specificationId: "spec", title: "API" } },
      global: { stubs: { OpenApiPreview: { name: "OpenApiPreview", props: ["editable"], template: '<div data-testid="preview" />' }, SpecificationHistory: true, OpenApiOperationCreateDialog: true, SpecificationCodeBindingDialog: true, CodeBindingDialog: true, Dialog: true } },
    });
    await flushPromises();
    const sourceCalls = mockWorkspace.loadSpecificationCodeBinding.mock.calls.length;
    const providerCalls = mockWorkspace.loadOpenApiProvider.mock.calls.length;
    const repositoryCalls = mockWorkspace.loadServiceRepository.mock.calls.length;
    expect(wrapper.findComponent({ name: "OpenApiPreview" }).props("editable")).toBe(true);
    let complete!: () => void;
    mockWorkspace.loadSpecificationCodeBinding.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = () => {
            binding.value = { path: "api.yaml" };
            resolve(binding.value);
          };
        }),
    );
    await wrapper.setProps({ catalog: { ...catalog } });
    expect(mockWorkspace.loadSpecificationCodeBinding).toHaveBeenCalledTimes(sourceCalls + 1);
    expect(mockWorkspace.loadOpenApiProvider).toHaveBeenCalledTimes(providerCalls + 1);
    expect(mockWorkspace.loadServiceRepository).toHaveBeenCalledTimes(repositoryCalls + 1);
    expect(wrapper.findComponent({ name: "OpenApiPreview" }).props("editable")).toBe(false);
    complete();
    await flushPromises();
    expect(wrapper.findComponent({ name: "OpenApiPreview" }).props("editable")).toBe(false);
    expect(wrapper.text()).toContain("由仓库托管");
    wrapper.unmount();
  });

  it("keeps whole-specification actions on the root when switching from source editing to an operation", async () => {
    const catalog = catalogFixture();
    catalog.specifications[0] = {
      ...catalog.specifications[0]!,
      kind: "openapi",
      content: JSON.stringify({ openapi: "3.0.3", info: { title: "API", version: "1" }, paths: { "/pets": { get: { responses: { "200": { description: "OK" } } } } } }),
    };
    mockWorkspace.specificationCodeBindingFor.mockReturnValue(null);
    mockWorkspace.specificationSyncFor.mockReturnValue(null);
    mockWorkspace.loadSpecificationCodeBinding.mockResolvedValue(null);
    const tab = { id: "specification-spec", kind: "specification" as const, serviceId: "svc-a", specificationId: "spec", title: "API" };
    const wrapper = mount(ServicePanel, {
      props: { catalog, serviceId: "svc-a", tab },
      global: {
        stubs: {
          OpenApiPreview: { template: '<div data-testid="operation-preview" />' },
          SpecificationEditor: { template: '<div data-testid="source-editor" />' },
          SpecificationHistory: true,
          SpecificationCodeBindingDialog: true,
          CodeBindingDialog: true,
          Dialog: true,
        },
      },
    });
    await flushPromises();
    expect(wrapper.find(".spec-toolbar").text()).toContain("删除规范");
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("编辑规范"))!
      .trigger("click");
    expect(wrapper.find('[data-testid="source-editor"]').exists()).toBe(true);

    await wrapper.setProps({ tab: { ...tab, id: "operation-get-pets", title: "GET /pets", operationKey: "GET:/pets" } });
    expect(wrapper.find(".spec-toolbar").exists()).toBe(false);
    expect(wrapper.find('[data-testid="source-editor"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="operation-preview"]').exists()).toBe(true);

    await wrapper.setProps({ tab });
    expect(wrapper.find('[data-testid="source-editor"]').exists()).toBe(true);
    wrapper.unmount();
  });
});
