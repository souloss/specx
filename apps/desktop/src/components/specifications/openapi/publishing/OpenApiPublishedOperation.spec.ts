import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import OpenApiPublishedOperation from "./OpenApiPublishedOperation.vue";
import { getOpenApiOperations } from "@/lib/specifications/openapi/contract/openapi";

const workbench = vi.hoisted(() => ({ mounts: vi.fn() }));
vi.mock("../OpenApiPreview.vue", () => ({
  default: {
    props: { publicTryIt: Boolean, embeddedWorkbench: Boolean, serviceId: String },
    setup: (props: unknown) => {
      workbench.mounts(props);
    },
    template: '<input aria-label="Request URL" />',
  },
}));
const document = { openapi: "3.1.0", paths: { "/pets": { get: { summary: "List pets", responses: {} } } } };
const content = JSON.stringify(document);
function setup(allowTryIt = true) {
  return mount(OpenApiPublishedOperation, { props: { content, document, entry: getOpenApiOperations(content)[0]!, title: "Pets", version: 1, allowTryIt, navigateModel: vi.fn() } });
}

describe("public operation page", () => {
  it("loads the isolated workbench only when requested and preserves the request across document tab switches", async () => {
    workbench.mounts.mockClear();
    const wrapper = setup();
    expect(workbench.mounts).not.toHaveBeenCalled();
    await wrapper.get('[role="tab"]').trigger("click");
    await flushPromises();
    expect(workbench.mounts).toHaveBeenCalledOnce();
    expect(workbench.mounts.mock.calls[0]![0]).toMatchObject({ publicTryIt: true, embeddedWorkbench: true, serviceId: undefined });
    await wrapper.get('input[aria-label="Request URL"]').setValue("https://api.example.test/pets");
    await wrapper.get('[role="tab"]').trigger("click");
    await wrapper.get('[role="tab"]:last-child').trigger("click");
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Request URL"]').element.value).toBe("https://api.example.test/pets");
    expect(workbench.mounts).toHaveBeenCalledOnce();
    await wrapper.setProps({ allowTryIt: false });
    expect(wrapper.find('input[aria-label="Request URL"]').exists()).toBe(false);
    expect(wrapper.find('[role="tab"]').exists()).toBe(false);
    expect(wrapper.get("h2").text()).toBe("List pets");
    wrapper.unmount();
  });

  it("shows only the documentation when online requests are disabled", () => {
    workbench.mounts.mockClear();
    const wrapper = setup(false);
    expect(wrapper.find('[role="tablist"]').exists()).toBe(false);
    expect(workbench.mounts).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("List pets");
    wrapper.unmount();
  });
});
