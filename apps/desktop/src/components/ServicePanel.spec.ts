import { flushPromises, mount } from "@vue/test-utils";
import { nextTick, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import ServicePanel from "./ServicePanel.vue";
import { catalogFixture } from "@/test/catalogFixture";

const mockWorkspace = vi.hoisted(() => ({
  specificationCodeBindingFor: vi.fn(),
  specificationSyncFor: vi.fn(),
  loadSpecificationCodeBinding: vi.fn(),
  syncSpecification: vi.fn(),
  deleteSpecification: vi.fn(),
  openOverview: vi.fn(),
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
    wrapper.unmount();
  });
});
