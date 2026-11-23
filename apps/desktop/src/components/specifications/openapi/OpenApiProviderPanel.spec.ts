import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import OpenApiProviderPanel from "./OpenApiProviderPanel.vue";

const mockWorkspace = vi.hoisted(() => ({
  createOpenApiEnvironment: vi.fn(),
  loadOpenApiProvider: vi.fn(),
  updateOpenApiEnvironment: vi.fn(),
  updateOpenApiSettings: vi.fn(),
  deleteOpenApiEnvironment: vi.fn(),
  enableOpenApiMock: vi.fn(),
  disableOpenApiMock: vi.fn(),
}));

vi.mock("@/stores/workspace", () => ({ useWorkspaceStore: () => mockWorkspace }));

const provider = {
  serviceId: "svc-api",
  settings: { defaultEnvironmentId: null, defaultServerUrl: "", showOperationId: true, schemaDisplay: "summary" as const, mockEnabled: false, updatedAt: "2026-11-09T00:00:00Z" },
  environments: [],
  specificationCount: 2,
  operationCount: 16,
  tagCount: 4,
};

describe("OpenAPI provider overview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
  });

  it("summarizes the provider and creates a labeled environment with variables", async () => {
    const wrapper = mount(OpenApiProviderPanel, {
      props: { serviceId: "svc-api", provider },
      global: {
        stubs: {
          Dialog: { template: "<div><slot /></div>" },
          DialogContent: { template: "<div><slot /></div>" },
          DialogFooter: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<div><slot /></div>" },
        },
      },
    });

    expect(wrapper.text()).toContain("16");
    expect(wrapper.text()).toContain("4");
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("新建环境"))!
      .trigger("click");
    await wrapper.get('[name="environment-name"]').setValue("QA");
    await wrapper.get('[name="environment-base-url"]').setValue("https://qa.example.test");
    const addVariable = wrapper.get('[aria-label="添加变量"]');
    await addVariable.setValue("tenant");
    await addVariable.trigger("keydown.enter");
    await wrapper.get('[name="environment-value-0"]').setValue("demo");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(mockWorkspace.createOpenApiEnvironment).toHaveBeenCalledWith(
      "svc-api",
      expect.objectContaining({
        name: "QA",
        baseUrl: "https://qa.example.test",
        variables: [expect.objectContaining({ key: "tenant", value: undefined, enabled: true, sensitive: false })],
      }),
    );
    expect(mockWorkspace.loadOpenApiProvider).toHaveBeenCalledWith("svc-api");
  });

  it("keeps Mock controls out of the provider UI even if enabled on the server", () => {
    const wrapper = mount(OpenApiProviderPanel, { props: { serviceId: "svc-api", provider: { ...provider, settings: { ...provider.settings, mockEnabled: true } } } });
    expect(wrapper.find("#openapi-mock-heading").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("Mock");
    expect(mockWorkspace.enableOpenApiMock).not.toHaveBeenCalled();
  });
});
