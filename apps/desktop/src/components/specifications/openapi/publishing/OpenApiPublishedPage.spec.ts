import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import OpenApiPublishedPage from "./OpenApiPublishedPage.vue";

const api = vi.hoisted(() => ({ getPublicOpenApiDocument: vi.fn() }));
vi.mock("@/lib/backend/client", () => ({ specxApi: api }));

const documentSnapshot = {
  publication: {
    id: "a".repeat(32),
    version: 4,
    title: "Public API",
    description: "Reference for integrators",
    theme: "dark" as const,
    showDownload: false,
    allowTryIt: true,
    createdAt: "2026-11-11T00:00:00Z",
  },
  content: '{"openapi":"3.1.0","info":{"title":"Public API","version":"1"},"paths":{}}',
};

function setup() {
  return mount(OpenApiPublishedPage, {
    props: { id: documentSnapshot.publication.id },
    global: {
      stubs: {
        Button: { template: "<button><slot /></button>" },
        SpecxMark: true,
        OpenApiDocumentReader: true,
      },
    },
  });
}

beforeEach(() => {
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) });
  document.documentElement.classList.remove("dark");
  document.documentElement.style.colorScheme = "";
  api.getPublicOpenApiDocument.mockResolvedValue(documentSnapshot);
});
afterEach(() => {
  document.documentElement.classList.remove("dark");
  document.documentElement.style.colorScheme = "";
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("published OpenAPI site", () => {
  it("opens search from the header or registered shortcut without intercepting a dialog", async () => {
    const focusSearch = vi.fn();
    const wrapper = mount(OpenApiPublishedPage, {
      props: { id: documentSnapshot.publication.id },
      global: { stubs: { SpecxMark: true, OpenApiDocumentReader: { template: "<div />", methods: { focusSearch } } } },
    });
    await flushPromises();
    await wrapper.get("button").trigger("click");
    expect(focusSearch).toHaveBeenCalledTimes(1);
    const event = new KeyboardEvent("keydown", { key: "k", ctrlKey: true, cancelable: true });
    window.dispatchEvent(event);
    expect(focusSearch).toHaveBeenCalledTimes(2);
    expect(event.defaultPrevented).toBe(true);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    document.body.append(dialog);
    dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }));
    expect(focusSearch).toHaveBeenCalledTimes(2);
    dialog.remove();
    wrapper.unmount();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }));
    expect(focusSearch).toHaveBeenCalledTimes(2);
  });

  it("persists a reader override and restores the workspace theme after closing", async () => {
    localStorage.setItem("specx-document-theme", "light");
    document.documentElement.classList.add("dark");
    document.documentElement.style.colorScheme = "dark";
    const wrapper = setup();
    await flushPromises();
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    await wrapper.get('button[aria-label="切换到深色主题"]').trigger("click");
    expect(localStorage.getItem("specx-document-theme")).toBe("dark");
    wrapper.unmount();
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });

  it("keeps reading and theme switching available when browser storage is blocked", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    const wrapper = setup();
    await flushPromises();
    expect(wrapper.text()).toContain("Public API");
    await wrapper.get('button[aria-label="切换到浅色主题"]').trigger("click");
    expect(document.documentElement.style.colorScheme).toBe("light");
    wrapper.unmount();
  });

  it("applies the saved reader theme and description and hides the download action when disabled", async () => {
    const wrapper = setup();
    await flushPromises();

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(wrapper.text()).toContain("Reference for integrators");
    expect(wrapper.text()).toContain("Public API");
    expect(wrapper.text()).not.toContain("导出文档");

    wrapper.unmount();
  });

  it("shows the definition download action when the publication allows it", async () => {
    api.getPublicOpenApiDocument.mockResolvedValue({
      ...documentSnapshot,
      publication: { ...documentSnapshot.publication, showDownload: true },
    });
    const wrapper = setup();
    await flushPromises();

    expect(wrapper.text()).toContain("导出文档");
    wrapper.unmount();
  });
});
