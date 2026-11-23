import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SpecificationDialog from "./SpecificationDialog.vue";
import { specxApi } from "@/lib/backend/client";
import { useWorkspaceStore } from "@/stores/workspace";
import { catalogFixture } from "@/test/catalogFixture";
import { installMemoryStorage } from "@/test/browserStorage";

let wrapper: VueWrapper;
let pinia: ReturnType<typeof createPinia>;
const createSpecification = vi.spyOn(specxApi, "createSpecification");

beforeEach(() => {
  installMemoryStorage();
  pinia = createPinia();
  setActivePinia(pinia);
  const workspace = useWorkspaceStore();
  workspace.catalog = catalogFixture();
  workspace.activeServiceId = "svc-a";
  vi.spyOn(workspace, "refresh").mockResolvedValue(undefined);
  createSpecification.mockReset().mockResolvedValue(catalogFixture().specifications[0]!);
});

afterEach(() => wrapper?.unmount());

describe("specification creation import flow", () => {
  it("imports a Markdown file by default, lets the user name it, and creates the specification", async () => {
    wrapper = mount(SpecificationDialog, {
      props: { open: false },
      global: {
        plugins: [pinia],
        stubs: {
          Dialog: { props: ["open"], template: "<div v-if='open'><slot /></div>" },
          DialogContent: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<header><slot /></header>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogFooter: { template: "<footer><slot /></footer>" },
        },
      },
    });
    await wrapper.setProps({ open: true });
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    const importMode = wrapper.findAll("button").find((button) => button.text().includes("从本地文件导入"));
    expect(importMode?.attributes("aria-pressed")).toBe("true");
    const input = wrapper.get('input[type="file"]');
    Object.defineProperty(input.element, "files", { configurable: true, value: [new File(["# Imported guide"], "guide.md", { type: "text/markdown" })] });
    await input.trigger("change");
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.text()).toContain("guide.md"));
    const titleInput = wrapper.get("label.settings-field input");
    await vi.waitFor(() => expect((titleInput.element as HTMLInputElement).value).toBe("guide"));
    await titleInput.setValue("Service guide");
    expect((titleInput.element as HTMLInputElement).value).toBe("Service guide");

    const nextButton = wrapper.findAll("button").find((button) => button.text().includes("下一步"));
    expect(nextButton?.attributes("disabled")).toBeUndefined();
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("guide.md");
    expect(wrapper.text()).toContain("Service guide");
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(createSpecification).toHaveBeenCalledWith("workspace", expect.objectContaining({ serviceId: "svc-a", kind: "markdown", title: "Service guide", content: "# Imported guide" }));
  });
});
