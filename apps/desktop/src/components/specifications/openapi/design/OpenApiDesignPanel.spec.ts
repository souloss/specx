import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { getOpenApiOperations, parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import OpenApiDesignPanel from "./OpenApiDesignPanel.vue";
import OpenApiDesignAddResponseDialog from "./OpenApiDesignAddResponseDialog.vue";
import OpenApiSchemaDesignEditor from "./OpenApiSchemaDesignEditor.vue";
import { installMemoryStorage } from "@/test/browserStorage";
import { openApiDesignStorageKey } from "@/lib/specifications/openapi/design/openapiDesignDraft";
import type { OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";

beforeEach(() => installMemoryStorage());
afterEach(() => vi.restoreAllMocks());

const document = {
  openapi: "3.0.3",
  info: { title: "Design contracts", version: "1" },
  paths: {
    "/records": {
      get: {
        parameters: [{ name: "filter", in: "query", schema: { type: "object", properties: { state: { type: "string" } } } }],
        responses: {
          "200": { description: "Records", headers: { "X-Trace": { schema: { type: "string" } } }, content: { "application/json": { schema: { type: "object", properties: { id: { type: "integer" } } } } } },
          "400": { description: "Invalid" },
        },
      },
      post: { responses: { "201": { description: "Created" } } },
    },
  },
};

function setup() {
  const save = vi.fn(async (_content: string, _change?: OpenApiOperationChange) => true);
  const wrapper = mount(OpenApiDesignPanel, { attachTo: globalThis.document.body, props: { content: JSON.stringify(document), focusOperationKey: "GET:/records", editable: true, saveOperation: save } });
  return { wrapper, save };
}

describe("OpenAPI operation contract design", () => {
  it("opens the first populated request tab and adds parameters from its heading action", async () => {
    const source = {
      openapi: "3.0.3",
      info: { title: "Path-first", version: "1" },
      paths: {
        "/records/{id}": {
          parameters: [{ name: "id", in: "path", required: true, schema: { type: "string" } }],
          get: { responses: { "200": { description: "Records" } } },
        },
      },
    };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records/{id}", editable: true, saveOperation: save } });
    const requestPanel = wrapper.get(".openapi-design-request-panel");
    expect(requestPanel.get('[role="tab"][aria-selected="true"]').attributes("data-value")).toBe("path");
    expect(requestPanel.find('[data-value="query"]').attributes("aria-selected")).toBe("false");

    await wrapper.setProps({ focusOperationKey: "GET:/records/{id}" });
    await requestPanel.get('[data-value="query"]').trigger("click");
    await requestPanel.get(".openapi-design-subsection-heading button").trigger("click");
    expect(wrapper.findAll<HTMLInputElement>('input[name^="parameter-name-"]').some((input) => input.element.value === "parameter1")).toBe(true);
    wrapper.unmount();
  });

  it("shows an actionable empty state for parameter groups without rows", async () => {
    const source = {
      openapi: "3.0.3",
      info: { title: "Empty groups", version: "1" },
      paths: { "/records": { get: { responses: { "204": { description: "No content" } } } } },
    };
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true } });
    const requestPanel = wrapper.get(".openapi-design-request-panel");
    expect(requestPanel.find(".openapi-design-table").exists()).toBe(false);
    const queryGroup = requestPanel.findAll(".openapi-design-parameter-group")[1]!;
    expect(queryGroup.get(".openapi-design-empty").text()).toContain("暂无 Query 参数");
    expect(queryGroup.get(".openapi-design-subsection-heading button").text()).toContain("添加 Query 参数");
    await queryGroup.get(".openapi-design-subsection-heading button").trigger("click");
    expect(queryGroup.find(".openapi-design-table").exists()).toBe(true);
    expect(queryGroup.find<HTMLInputElement>('input[name^="parameter-name-"]').element.value).toBe("parameter1");
    wrapper.unmount();
  });

  it("selects the invalid parameter tab before focusing its name field", async () => {
    const { wrapper } = setup();
    const requestPanel = wrapper.get(".openapi-design-request-panel");
    await requestPanel.get('[data-value="query"]').trigger("click");
    const queryGroup = wrapper.get('[data-parameter-location="query"]');
    await queryGroup.get(".openapi-design-subsection-heading button").trigger("click");
    const invalidName = queryGroup.findAll<HTMLInputElement>('input[name^="parameter-name-"]').at(-1)!;
    await invalidName.setValue("");
    await requestPanel.get('[data-value="path"]').trigger("click");

    await wrapper.get(".openapi-design-actions button").trigger("click");

    expect(requestPanel.get('[data-value="query"]').attributes("aria-selected")).toBe("true");
    expect(globalThis.document.activeElement).toBe(invalidName.element);
    wrapper.unmount();
  });

  it("selects and focuses the response with an invalid status", async () => {
    const { wrapper } = setup();
    const invalidStatus = wrapper.get('[data-response-status="400"] input[name^="openapi-response-status-"]');
    await invalidStatus.setValue("invalid");
    await wrapper.get('.openapi-response-status-tabs [data-value="response:200"]').trigger("click");

    await wrapper.get(".openapi-design-actions button").trigger("click");

    expect(wrapper.get('.openapi-response-status-tabs [data-value="response:400"]').attributes("aria-selected")).toBe("true");
    expect(globalThis.document.activeElement).toBe(invalidStatus.element);
    wrapper.unmount();
  });

  it("selects and focuses the response with an invalid media type", async () => {
    const { wrapper } = setup();
    await wrapper.get('select[name="openapi-response-format-response:200"]').setValue("__custom");
    const invalidMediaType = wrapper.get('input[name="openapi-response-media-type-200"]');
    await invalidMediaType.setValue("invalid");
    await invalidMediaType.trigger("keydown", { key: "Enter" });
    await wrapper.get('.openapi-response-status-tabs [data-value="response:400"]').trigger("click");

    await wrapper.get(".openapi-design-actions button").trigger("click");

    expect(wrapper.get('.openapi-response-status-tabs [data-value="response:200"]').attributes("aria-selected")).toBe("true");
    expect(globalThis.document.activeElement).toBe(invalidMediaType.element);
    wrapper.unmount();
  });

  it("opens the response headers section before focusing an invalid header", async () => {
    const { wrapper } = setup();
    const headerName = wrapper.get('[data-response-status="200"] .openapi-design-response-headers input');
    await headerName.setValue("");
    await wrapper.get('.openapi-response-status-tabs [data-value="response:400"]').trigger("click");

    await wrapper.get(".openapi-design-actions button").trigger("click");

    expect(wrapper.get('.openapi-response-status-tabs [data-value="response:200"]').attributes("aria-selected")).toBe("true");
    expect(wrapper.get('[data-response-status="200"] .openapi-design-response-headers').attributes("open")).toBeDefined();
    expect(globalThis.document.activeElement).toBe(headerName.element);
    wrapper.unmount();
  });

  it("saves a preset media type and a custom vendor media type without affecting other operations", async () => {
    const { wrapper, save } = setup();
    const response = wrapper.get('[data-response-status="200"]');
    const format = response.get('select[name="openapi-response-format-response:200"]');
    await format.setValue("text/plain");
    await response.get('textarea[name="openapi-response-body-response:200"]').setValue("Records exported");
    await format.setValue("");
    expect(response.find(".openapi-response-design-body").exists()).toBe(false);
    await format.setValue("text/plain");
    expect(response.get('textarea[name="openapi-response-body-response:200"]').element).toHaveProperty("value", "Records exported");
    await format.setValue("__custom");
    await response.get('input[name="openapi-response-media-type-200"]').setValue("application/vnd.specx+json");
    await response.get('input[name="openapi-response-media-type-200"]').trigger("keydown", { key: "Enter" });
    await wrapper.get(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const actual = parseOpenApiContent(save.mock.calls[0]![0])!;
    const entries = getOpenApiOperations(JSON.stringify(actual));
    const operation = entries.find((item) => item.identityKey === "GET:/records")!.operation;
    expect((operation.responses as (typeof document)["paths"]["/records"]["get"]["responses"])["200"].content).toHaveProperty("text/plain");
    expect((operation.responses as (typeof document)["paths"]["/records"]["get"]["responses"])["200"].content).toMatchObject({ "text/plain": { example: "Records exported" } });
    expect((operation.responses as (typeof document)["paths"]["/records"]["get"]["responses"])["200"].content).toHaveProperty("application/json");
    expect((operation.responses as (typeof document)["paths"]["/records"]["get"]["responses"])["200"].content).toHaveProperty("application/vnd.specx+json");
    expect(entries.find((item) => item.identityKey === "POST:/records")!.operation).toEqual(document.paths["/records"].post);
    wrapper.unmount();
  });

  it("preserves a pending response when extraction targets the same unsaved status", async () => {
    const { wrapper, save } = setup();
    await wrapper.get('[data-response-status="400"] input[aria-label="响应状态码"]').setValue("422");
    await wrapper.setProps({ capturedExample: { name: "captured", body: '{"field":1}', status: "422", mediaType: "application/json", action: "newResponse", infer: true, operationKey: "GET:/records" } });
    expect(wrapper.findAll('[data-response-status="422"]')).toHaveLength(1);
    expect(wrapper.get('[data-response-status="422"]').text()).not.toContain("captured");
    await wrapper.get(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const actual = parseOpenApiContent(save.mock.calls[0]![0]);
    expect(getOpenApiOperations(JSON.stringify(actual)).find((item) => item.identityKey === "GET:/records")?.operation.responses).toEqual({ "200": document.paths["/records"].get.responses["200"], "422": { description: "Invalid" } });
    wrapper.unmount();
  });

  it("saves explicit reference detachment even without further field edits", async () => {
    const model = { type: "string", minLength: 3 };
    const source = { openapi: "3.1.0", info: { title: "API", version: "1" }, components: { schemas: { Name: model } }, paths: { "/records": { get: { responses: { "200": { description: "OK", content: { "application/json": { schema: { $ref: "#/components/schemas/Name", maxLength: 10 } } } } } } } } };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true, saveOperation: save } });
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "展开为独立结构")!
      .trigger("click");
    expect(wrapper.find(".schema-design-reference-source").exists()).toBe(false);
    expect(wrapper.find(".openapi-design-actions button:last-child").attributes("disabled")).toBeUndefined();
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toEqual({ ...source, paths: { "/records": { get: { responses: { "200": { description: "OK", content: { "application/json": { schema: { allOf: [model, { maxLength: 10 }] } } } } } } } } });
    wrapper.unmount();
  });

  it("reuses models in parameter schemas while preserving inherited and referenced parameter objects", async () => {
    const parameters = [{ name: "filter", in: "query", schema: { $ref: "#/components/schemas/Filter" } }, { $ref: "#/components/parameters/Trace" }, { name: "encoded", in: "query", content: { "application/json": { schema: { type: "string" } } } }];
    const inherited = [{ name: "shared", in: "header", schema: { type: "string" } }];
    const source = {
      ...document,
      components: { schemas: { Filter: { type: "object", properties: { state: { type: "string" } } }, Name: { type: "string" } }, parameters: { Trace: { name: "trace", in: "header", schema: { type: "string" } } } },
      paths: { "/records": { ...document.paths["/records"], parameters: inherited, get: { ...document.paths["/records"].get, parameters } } },
    };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true, saveOperation: save } });
    const type = wrapper.get('select[name="parameter-type-operation:query:filter:0"]');
    expect(type.element).toHaveProperty("value", "#/components/schemas/Filter");
    await type.setValue("__model");
    await flushPromises();
    const picker = new DOMWrapper(globalThis.document.body).get(".schema-model-picker");
    await picker.get('input[name="openapi-model-search"]').setValue("Name");
    await picker.get('input[aria-label="Name"]').setValue(true);
    await picker.get('input[name="schema-minLength-parameter-operation:query:filter:0"]').setValue("2");
    await picker.get(".border-t button:last-child").trigger("click");
    await flushPromises();
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toEqual({
      ...source,
      paths: { "/records": { ...source.paths["/records"], get: { ...source.paths["/records"].get, parameters: [{ ...parameters[0], schema: { allOf: [{ $ref: "#/components/schemas/Name" }], minLength: 2 } }, ...parameters.slice(1)] } } },
    });
    wrapper.unmount();
  });

  it("preserves original reference siblings for metadata edits and applies model choices only to the selected response", async () => {
    const schema = { $ref: "#/components/schemas/Record", maxProperties: 8 };
    const source = { ...structuredClone(document), openapi: "3.1.0", components: { schemas: { Record: { type: "object", properties: { id: { type: "integer" } } }, Error: { type: "object", properties: { message: { type: "string" } } } } } };
    source.paths["/records"].get.responses["200"].content["application/json"].schema = schema as unknown as (typeof document)["paths"]["/records"]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true, saveOperation: save, draftScope: "w:s" } });
    const editor = wrapper.findComponent(OpenApiSchemaDesignEditor);
    expect(editor.props("modelValue")).toEqual(schema);
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Metadata only");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toMatchObject({ paths: { "/records": { get: { responses: { "200": { content: { "application/json": { schema } } } } } } } });
    const savedContent = save.mock.calls[0]![0];
    await wrapper.setProps({ content: savedContent });
    editor.vm.$emit("update:modelValue", { $ref: "#/components/schemas/Error" });
    await flushPromises();
    wrapper.unmount();
    const restored = mount(OpenApiDesignPanel, { props: { content: savedContent, focusOperationKey: "GET:/records", editable: true, saveOperation: save, draftScope: "w:s" } });
    expect(restored.findComponent(OpenApiSchemaDesignEditor).props("modelValue")).toEqual({ $ref: "#/components/schemas/Error" });
    await restored.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const expected = structuredClone(source);
    Object.assign(expected.paths["/records"].get, { summary: "Metadata only" });
    expected.paths["/records"].get.responses["200"].content["application/json"].schema = { $ref: "#/components/schemas/Error" } as unknown as (typeof source)["paths"]["/records"]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
    expect(parseOpenApiContent(save.mock.calls[1]![0])).toEqual(expected);
    restored.unmount();
  });

  it("allows replacing an unresolved schema with a local model without editing shared response containers", async () => {
    const source = {
      ...document,
      components: { schemas: { Name: { type: "string" }, Never: false }, responses: { Shared: { description: "Shared", content: { "application/json": { schema: { $ref: "https://external.test/schema" } } } } } },
      paths: { "/records": { get: { responses: { "200": { $ref: "#/components/responses/Shared" } } }, post: document.paths["/records"].post } },
    };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true, saveOperation: save } });
    const editor = wrapper.findComponent(OpenApiSchemaDesignEditor);
    expect(editor.exists()).toBe(true);
    editor.vm.$emit("update:modelValue", { $ref: "#/components/schemas/Name" });
    await flushPromises();
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toEqual({ ...source, paths: { "/records": { ...source.paths["/records"], get: { responses: { "200": { description: "Shared", content: { "application/json": { schema: { $ref: "#/components/schemas/Name" } } } } } } } } });
    wrapper.unmount();
  });

  it("validates response status changes and moves the entire response without changing other operations", async () => {
    const { wrapper, save } = setup();
    const status = wrapper.findAll('input[aria-label="响应状态码"]')[0]!;
    await status.setValue("400");
    expect(wrapper.find('[role="alert"]').text()).toContain("未重复");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    expect(save).not.toHaveBeenCalled();
    await status.setValue("600");
    expect(wrapper.find('[role="alert"]').exists()).toBe(true);
    await status.setValue("202");
    await wrapper.find('textarea[name="schema-description-1-id"]').setValue("Deferred record ID");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const saved = parseOpenApiContent(save.mock.calls[0]![0])!;
    const operation = getOpenApiOperations(save.mock.calls[0]![0]).find((entry) => entry.method === "get")!.operation;
    expect(operation.responses).not.toHaveProperty("200");
    expect(operation.responses).toMatchObject({ "202": { description: "Records", headers: document.paths["/records"].get.responses["200"].headers, content: { "application/json": { schema: { properties: { id: { description: "Deferred record ID" } } } } } } });
    expect(getOpenApiOperations(JSON.stringify(saved)).find((entry) => entry.method === "post")!.operation).toEqual(document.paths["/records"].post);
    wrapper.unmount();
  });

  it("removes the body when changing a response to 204 and keeps response headers", async () => {
    const { wrapper, save } = setup();
    await wrapper.findAll('input[aria-label="响应状态码"]')[0]!.setValue("204");
    expect(wrapper.find('textarea[name="schema-description-1-id"]').exists()).toBe(false);
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const operation = getOpenApiOperations(save.mock.calls[0]![0]).find((entry) => entry.method === "get")!.operation;
    expect(operation.responses).toEqual({ "204": { description: "Records", headers: document.paths["/records"].get.responses["200"].headers }, "400": { description: "Invalid" } });
    wrapper.unmount();
  });

  it("saves parameter serialization choices and resets incompatible styles when changing location", async () => {
    const { wrapper, save } = setup();
    await wrapper.get('.openapi-design-parameter-group [aria-haspopup="dialog"]').trigger("click");
    const dialog = new DOMWrapper(globalThis.document.body).get(".schema-field-dialog");
    await dialog.get('select[name^="parameter-style-"]').setValue("deepObject");
    await dialog.get('select[name^="parameter-explode-"]').setValue("true");
    await dialog.get('input[name^="parameter-allow-reserved-"]').setValue(true);
    await dialog.get(".border-t button:last-child").trigger("click");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const operation = getOpenApiOperations(save.mock.calls[0]![0]).find((entry) => entry.method === "get")!.operation;
    expect(operation.parameters).toEqual([{ ...document.paths["/records"].get.parameters[0], style: "deepObject", explode: true, allowReserved: true }]);
    await wrapper.find('button[aria-label="位置"] + select.app-select-native').setValue("header");
    await wrapper.get('[data-parameter-location="header"] [aria-haspopup="dialog"]').trigger("click");
    const movedDialog = new DOMWrapper(globalThis.document.body).get(".schema-field-dialog");
    expect(movedDialog.get('select[name^="parameter-style-"]').element).toHaveProperty("value", "");
    expect(movedDialog.find('input[name^="parameter-allow-reserved-"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it("recovers independent operation drafts, nested schema changes and invalid partial inputs after remount", async () => {
    const props = { content: JSON.stringify(document), focusOperationKey: "GET:/records", draftScope: "workspace:spec", editable: true };
    const wrapper = mount(OpenApiDesignPanel, { props });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Local GET");
    await wrapper.find('textarea[name="schema-description-1-id"]').setValue("Local ID description");
    await wrapper.findAll('input[aria-label="响应状态码"]')[0]!.setValue("20");
    await wrapper.setProps({ focusOperationKey: "POST:/records" });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Local POST");
    await wrapper.setProps({ focusOperationKey: "GET:/records" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Local GET");
    expect(wrapper.findAll<HTMLInputElement>('input[aria-label="响应状态码"]')[0]!.element.value).toBe("20");
    wrapper.unmount();

    const restored = mount(OpenApiDesignPanel, { props });
    expect(restored.find<HTMLTextAreaElement>('textarea[name="schema-description-1-id"]').element.value).toBe("Local ID description");
    expect(restored.findAll<HTMLInputElement>('input[aria-label="响应状态码"]')[0]!.element.value).toBe("20");
    expect(restored.find(".openapi-design-actions button:first-of-type").attributes("disabled")).toBeUndefined();
    await restored.setProps({ focusOperationKey: "POST:/records" });
    expect(restored.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Local POST");
    restored.unmount();
  });

  it("rebases a draft over changes to sibling operations and saves against the latest document", async () => {
    const { wrapper, save } = setup();
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Local GET");
    const remote = structuredClone(document);
    remote.paths["/records"].post.responses["201"].description = "Created remotely";
    await wrapper.setProps({ content: JSON.stringify(remote) });
    expect(wrapper.find(".openapi-design-conflict").exists()).toBe(false);
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Local GET");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const saved = parseOpenApiContent(save.mock.calls[0]![0]);
    expect(saved).toMatchObject({ paths: { "/records": { get: { summary: "Local GET" }, post: remote.paths["/records"].post } } });
    wrapper.unmount();
  });

  it("preserves local values and blocks overwrite when the selected operation changed remotely", async () => {
    const { wrapper, save } = setup();
    await wrapper.setProps({ draftScope: "workspace:spec" });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Local summary");
    const remote = structuredClone(document);
    remote.paths["/records"].get.responses["200"].description = "Remote response";
    await wrapper.setProps({ content: JSON.stringify(remote) });
    expect(wrapper.find(".openapi-design-conflict").text()).toContain("草稿已保留");
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Local summary");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    expect(save).not.toHaveBeenCalled();
    expect(localStorage.getItem(openApiDesignStorageKey("workspace:spec", "GET:/records"))).toContain("Local summary");
    await wrapper.find(".openapi-design-conflict button:last-child").trigger("click");
    expect(wrapper.find(".openapi-design-conflict").exists()).toBe(false);
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("");
    expect(localStorage.getItem(openApiDesignStorageKey("workspace:spec", "GET:/records"))).toBeNull();
    wrapper.unmount();
  });

  it("isolates persisted drafts between workspaces and specifications", async () => {
    const { wrapper } = setup();
    await wrapper.setProps({ draftScope: "workspace:spec-a" });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Spec A draft");
    await wrapper.setProps({ draftScope: "workspace:spec-b" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("");
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Spec B draft");
    await wrapper.setProps({ draftScope: "workspace:spec-a" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Spec A draft");
    await wrapper.setProps({ draftScope: "another-workspace:spec-a" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("");
    wrapper.unmount();
  });

  it("keeps drafts in memory if browser storage is unavailable and ignores corrupt persisted state", async () => {
    const key = openApiDesignStorageKey("workspace:spec", "GET:/records");
    localStorage.setItem(key, JSON.stringify({ baseContext: "old", state: { summary: "Invalid state" } }));
    const { wrapper } = setup();
    await wrapper.setProps({ draftScope: "workspace:spec" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("");
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new Error("Quota exceeded");
    });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("In memory");
    await wrapper.setProps({ focusOperationKey: "POST:/records" });
    await wrapper.setProps({ focusOperationKey: "GET:/records" });
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("In memory");
    expect(wrapper.text()).not.toContain("草稿已保存在此设备");
    wrapper.unmount();
  });

  it("requires review when shared models change while a design draft is open", async () => {
    const { wrapper } = setup();
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Local GET");
    await wrapper.setProps({ content: JSON.stringify({ ...document, components: { schemas: { Record: { type: "string" } } } }) });
    expect(wrapper.find(".openapi-design-conflict").exists()).toBe(true);
    expect(wrapper.find(".openapi-design-actions button:last-child").attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("edits the operation's method and path in the same save and provides its new navigation identity", async () => {
    const { wrapper, save } = setup();
    await wrapper.find('select[name="openapi-design-method"]').setValue("put");
    await wrapper.find('input[name="openapi-design-path"]').setValue("/records/{recordId}");
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Update one record");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    const saved = parseOpenApiContent(save.mock.calls[0]![0])!;
    expect(saved).toMatchObject({
      paths: { "/records": { post: document.paths["/records"].post }, "/records/{recordId}": { put: { summary: "Update one record", parameters: [document.paths["/records"].get.parameters[0], { name: "recordId", in: "path", required: true, schema: { type: "string" } }] } } },
    });
    expect((saved.paths as Record<string, unknown>)["/records"]).not.toHaveProperty("get");
    expect(save.mock.calls[0]![1]).toEqual({ previousKey: "GET:/records", key: "PUT:/records/{recordId}", title: "PUT /records/{recordId}" });
    wrapper.unmount();
  });

  it("preserves edited path parameter details and references when renaming a route", async () => {
    const source = {
      openapi: "3.0.3",
      info: { title: "API", version: "1" },
      paths: {
        "/records/{id}": { get: { parameters: [{ name: "id", in: "path", required: true, schema: { type: "string" } }], responses: { "200": { description: "OK" } } } },
        "/other": { get: { parameters: [{ $ref: "#/paths/~1records~1{id}/get/parameters/0" }], responses: { "200": { description: "OK" } } } },
      },
    };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records/{id}", editable: true, saveOperation: save } });
    await wrapper.get('input[name="openapi-design-path"]').setValue("/records/{recordId}");
    const group = wrapper.findAll(".openapi-design-parameter-group")[0]!;
    expect(group.text()).toContain("recordId");
    await group.get('select[name^="parameter-type-"]').setValue("integer");
    await group.get('input[aria-label="说明"]').setValue("Stable ID");
    await wrapper.get(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();
    expect(parseOpenApiContent(save.mock.calls[0]![0])).toMatchObject({
      paths: {
        "/records/{recordId}": { get: { parameters: [{ name: "recordId", in: "path", required: true, description: "Stable ID", schema: { type: "integer" } }] } },
        "/other": { get: { parameters: [{ $ref: "#/paths/~1records~1{recordId}/get/parameters/0" }] } },
      },
    });
    wrapper.unmount();
  });

  it("prevents duplicate routes and preserves a device draft from before address editing existed", async () => {
    const { wrapper, save } = setup();
    await wrapper.setProps({ draftScope: "workspace:spec" });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Legacy local draft");
    wrapper.unmount();
    const key = openApiDesignStorageKey("workspace:spec", "GET:/records");
    const persisted = JSON.parse(localStorage.getItem(key)!);
    delete persisted.state.method;
    delete persisted.state.path;
    localStorage.setItem(key, JSON.stringify(persisted));
    const restored = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(document), focusOperationKey: "GET:/records", draftScope: "workspace:spec", editable: true, saveOperation: save } });
    expect(restored.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Legacy local draft");
    expect(restored.find<HTMLInputElement>('input[name="openapi-design-path"]').element.value).toBe("/records");
    await restored.find('select[name="openapi-design-method"]').setValue("post");
    expect(restored.text()).toContain("此路径和方法已存在");
    await restored.find(".openapi-design-actions button:last-child").trigger("click");
    expect(save).not.toHaveBeenCalled();
    restored.unmount();
  });

  it("does not show a late save result on another operation or clear its draft", async () => {
    let complete!: (value: boolean) => void;
    const save = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          complete = resolve;
        }),
    );
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(document), focusOperationKey: "GET:/records", draftScope: "workspace:spec", editable: true, saveOperation: save } });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Saved GET");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await wrapper.setProps({ focusOperationKey: "POST:/records" });
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Unsaved POST");
    complete(true);
    await flushPromises();
    expect(wrapper.text()).not.toContain("接口设计已保存");
    expect(wrapper.find<HTMLInputElement>('input[name="openapi-design-summary"]').element.value).toBe("Unsaved POST");
    expect(localStorage.getItem(openApiDesignStorageKey("workspace:spec", "GET:/records"))).toBeNull();
    wrapper.unmount();
    expect(localStorage.getItem(openApiDesignStorageKey("workspace:spec", "POST:/records"))).toContain("Unsaved POST");
  });

  it("shows request parameter locations as ordered tabs and adds a response in its own tab", async () => {
    const { wrapper } = setup();
    const requestTabs = wrapper.get('[role="tablist"][aria-label="请求参数"]');
    expect(requestTabs.findAll('[role="tab"]').map((tab) => tab.attributes("data-value"))).toEqual(["path", "query", "header", "cookie", "body"]);

    const addResponseDialog = wrapper.findComponent(OpenApiDesignAddResponseDialog);
    addResponseDialog.vm.$emit("add", { status: "422", description: "Unprocessable Content", mediaType: "application/json" });
    await flushPromises();

    expect(wrapper.find('[role="tablist"][aria-label="返回响应"] [aria-selected="true"]').text()).toContain("422");
    expect(wrapper.find('[data-response-status="422"]').exists()).toBe(true);
    expect(wrapper.find('[data-response-status="422"] .openapi-design-schema-card code').text()).toBe("application/json");
    wrapper.unmount();
  });

  it("hides the deprecated label while preserving imported operation metadata on save", async () => {
    const source = {
      ...document,
      paths: {
        ...document.paths,
        "/records": { ...document.paths["/records"], get: { ...document.paths["/records"].get, deprecated: true } },
      },
    };
    const save = vi.fn(async (_content: string) => true);
    const wrapper = mount(OpenApiDesignPanel, { props: { content: JSON.stringify(source), focusOperationKey: "GET:/records", editable: true, saveOperation: save } });

    expect(wrapper.text()).not.toContain("已弃用");
    await wrapper.find('input[name="openapi-design-summary"]').setValue("Updated summary");
    await wrapper.find(".openapi-design-actions button:last-child").trigger("click");
    await flushPromises();

    expect(parseOpenApiContent(save.mock.calls[0]![0])?.paths).toMatchObject({ "/records": { get: { deprecated: true, summary: "Updated summary" } } });
    wrapper.unmount();
  });
});
