import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { installMemoryStorage } from "@/test/browserStorage";
import { parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import { validateOpenApiSchemaInWorker } from "@/lib/specifications/openapi/contract/openapiSchemaValidationClient";
import type { OpenApiModelChange } from "@/lib/specifications/openapi/contract/openapiModels";
import OpenApiModelsPanel from "./OpenApiModelsPanel.vue";
vi.mock("@/lib/specifications/openapi/contract/openapiSchemaValidationClient", () => ({ validateOpenApiSchemaInWorker: vi.fn(async () => ({ valid: false, issues: [] })) }));
const document = {
  openapi: "3.1.0",
  info: { title: "API", version: "1" },
  paths: { "/pets": { get: { responses: { "200": { description: "OK", content: { "application/json": { schema: { $ref: "#/components/schemas/Pet" } } } } } } } },
  components: { schemas: { Pet: { type: "object", properties: { name: { type: "string" } } }, Other: { type: "integer" } } },
};
const content = JSON.stringify(document);
const reference = "#/components/schemas/Pet";
beforeEach(() => {
  installMemoryStorage();
  vi.clearAllMocks();
});
function setup(saveDocument = vi.fn(async (_content: string, _change?: OpenApiModelChange) => true)) {
  return mount(OpenApiModelsPanel, { props: { content, modelRef: reference, specificationTitle: "API", editable: true, draftScope: "workspace/spec", saveDocument, navigateModel: vi.fn(), navigateOperation: vi.fn() }, global: { stubs: { OpenApiCodeEditor: true } } });
}
async function design(wrapper: ReturnType<typeof setup>) {
  await wrapper.findAll('[role="tab"]')[1]!.trigger("mousedown", { button: 0 });
  await flushPromises();
}
function saveButton(wrapper: ReturnType<typeof setup>) {
  return wrapper.findAll("button").find((button) => button.text() === "保存")!;
}

describe("shared model design", () => {
  it("saves only the selected schema, rewrites references on rename and retains inputs for retry", async () => {
    const save = vi.fn(async (_content: string, _change?: OpenApiModelChange) => true).mockRejectedValueOnce(new Error("Offline"));
    const wrapper = setup(save);
    await design(wrapper);
    await wrapper.find('input[name="openapi-model-name"]').setValue("PetRecord");
    await wrapper.find('textarea[name="schema-description-1-name"]').setValue("Shared name");
    await saveButton(wrapper).trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Offline");
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-model-name"]').element.value).toBe("PetRecord");
    await saveButton(wrapper).trigger("click");
    await flushPromises();
    expect(save).toHaveBeenCalledTimes(2);
    const saved = parseOpenApiContent(save.mock.calls[1]![0]);
    expect(saved).toMatchObject({
      components: { schemas: { PetRecord: { type: "object", properties: { name: { type: "string", description: "Shared name" } } }, Other: { type: "integer" } } },
      paths: { "/pets": { get: { responses: { "200": { content: { "application/json": { schema: { $ref: "#/components/schemas/PetRecord" } } } } } } } },
    });
    expect(save.mock.calls[1]![1]).toEqual({ kind: "model", previousReference: reference, reference: "#/components/schemas/PetRecord", title: "PetRecord" });
    wrapper.unmount();
  });

  it("restores model drafts after remount and blocks overwriting changed shared definitions", async () => {
    let wrapper = setup();
    await design(wrapper);
    await wrapper.find('textarea[name="schema-description-1-name"]').setValue("Local draft");
    wrapper.unmount();
    wrapper = setup();
    await design(wrapper);
    expect(wrapper.find<HTMLTextAreaElement>('textarea[name="schema-description-1-name"]').element.value).toBe("Local draft");
    await wrapper.setProps({ content: JSON.stringify({ ...document, components: { schemas: { ...document.components.schemas, Pet: { type: "string" } } } }) });
    expect(wrapper.text()).toContain("公共模型定义已更新");
    expect(saveButton(wrapper).attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("rechecks the source after async validation so a newer ETag cannot save a stale whole document", async () => {
    let complete!: (value: { valid: boolean; issues: [] }) => void;
    vi.mocked(validateOpenApiSchemaInWorker).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const save = vi.fn(async (_content: string, _change?: OpenApiModelChange) => true);
    const wrapper = setup(save);
    await design(wrapper);
    await wrapper.find('textarea[name="schema-description-1-name"]').setValue("Local draft");
    await saveButton(wrapper).trigger("click");
    await wrapper.setProps({ content: JSON.stringify({ ...document, info: { title: "Concurrent", version: "2" } }) });
    complete({ valid: false, issues: [] });
    await flushPromises();
    expect(save).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("规范已有新版本");
    expect(wrapper.find<HTMLTextAreaElement>('textarea[name="schema-description-1-name"]').element.value).toBe("Local draft");
    wrapper.unmount();
  });
});
