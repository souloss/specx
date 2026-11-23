import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import OpenApiDocumentReader from "./OpenApiDocumentReader.vue";

const content = JSON.stringify({
  openapi: "3.1.0",
  info: { description: "Reader overview" },
  paths: { "/pets": { get: { summary: "List pets", operationId: "listPets", tags: ["Animals"], responses: {} }, post: { summary: "Create pet", tags: ["Animals"], responses: {} } }, "/status": { get: { responses: {} } } },
  components: { schemas: { Pet: { type: "object", description: "A pet", required: ["id"], properties: { id: { type: "string" }, name: { type: "string" } } } } },
});
function setup(linked = false) {
  return mount(OpenApiDocumentReader, { props: { content, title: "Pets", version: 4, linked }, global: { stubs: { OpenApiPublishedOperation: { props: ["entry"], template: "<div data-operation>{{ entry.identityKey }}</div>" } } } });
}
beforeEach(() => Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() }));
afterEach(() => vi.unstubAllGlobals());
describe("published operation navigation", () => {
  it("switches to a model catalogue and renders the model fields", async () => {
    const wrapper = setup(true);
    await wrapper.get("button").trigger("click");
    const modelsTab = wrapper.findAll("button").find((entry) => entry.text() === "数据模型")!;
    await modelsTab.trigger("click");
    expect(wrapper.text()).toContain("Pet");
    await wrapper
      .findAll("aside a")
      .find((entry) => entry.text().includes("Pet"))!
      .trigger("click");
    expect(wrapper.text()).toContain("id");
    expect(wrapper.text()).toContain("name");
    wrapper.unmount();
  });

  it("searches metadata and tags without replacing the selected operation", async () => {
    const wrapper = setup(true);
    await wrapper.get('input[type="search"]').setValue("listPets");
    expect(wrapper.findAll("aside li a")).toHaveLength(1);
    expect(wrapper.get("aside li a").attributes("href")).toBe("#GET%3A%2Fpets");
    await wrapper.get("aside li a").trigger("click");
    expect(wrapper.get("[data-operation]").text()).toBe("GET:/pets");
    expect(wrapper.get("aside li a").attributes("aria-current")).toBe("page");
    await wrapper.get('input[type="search"]').setValue("animals");
    expect(wrapper.findAll("aside li a")).toHaveLength(2);
    await wrapper.get('input[type="search"]').setValue("missing");
    expect(wrapper.text()).toContain("没有匹配的接口");
    expect(wrapper.get("[data-operation]").text()).toBe("GET:/pets");
    wrapper.unmount();
  });

  it("collapses the narrow catalogue on selection and preserves modified link clicks", async () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    const wrapper = setup(true);
    const link = wrapper.findAll("aside a").find((entry) => entry.text().includes("POST"))!;
    await link.trigger("click", { ctrlKey: true });
    expect(wrapper.emitted("update:operation")).toBeUndefined();
    await link.trigger("click");
    expect(wrapper.get("[aria-expanded]").attributes("aria-expanded")).toBe("false");
    expect(wrapper.get("[data-operation]").text()).toBe("POST:/pets");
    wrapper.unmount();
  });

  it("opens a tag landing page and keeps field search across catalogues", async () => {
    const wrapper = setup(true);
    await wrapper.get("summary a").trigger("click");
    expect(wrapper.emitted("update:operation")?.at(-1)).toEqual(["tag:Animals"]);
    expect(wrapper.get("h2").text()).toBe("Animals");
    expect(wrapper.get("summary a").attributes("aria-current")).toBe("page");
    await wrapper.get('input[type="search"]').setValue("name");
    const modelsTab = wrapper.findAll("button").find((entry) => entry.text().startsWith("数据模型"))!;
    await modelsTab.trigger("click");
    expect(wrapper.get<HTMLInputElement>('input[type="search"]').element.value).toBe("name");
    expect(wrapper.findAll("aside a").some((entry) => entry.text().includes("Pet"))).toBe(true);
    wrapper.unmount();
  });
});
