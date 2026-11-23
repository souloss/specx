import { describe, expect, it } from "vitest";
import { DOMWrapper, mount } from "@vue/test-utils";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import OpenApiSchemaDesignEditor from "./OpenApiSchemaDesignEditor.vue";

function setup(schema: OpenApiObject, document: OpenApiObject = { openapi: "3.0.3" }) {
  const wrapper = mount(OpenApiSchemaDesignEditor, {
    props: {
      modelValue: structuredClone(schema),
      document,
      label: "Value",
      "onUpdate:modelValue": (value: OpenApiObject): void => {
        void wrapper.setProps({ modelValue: value });
      },
    },
  });
  return wrapper;
}

async function openSettings(wrapper: ReturnType<typeof setup>) {
  await wrapper.get('[aria-haspopup="dialog"]').trigger("click");
  return new DOMWrapper(document.body).get(".schema-field-dialog");
}

async function applySettings(dialog: Awaited<ReturnType<typeof openSettings>>) {
  await dialog.get(".border-t button:last-child").trigger("click");
}

describe("OpenAPI schema design", () => {
  it("edits nullable and field direction flags using the OpenAPI 3.0 representation", async () => {
    const wrapper = setup({ type: "string", deprecated: true });
    const dialog = await openSettings(wrapper);
    await dialog.get('input[name="schema-nullable-0-Value"]').setValue(true);
    await dialog.get('button[aria-label="读写行为"] + select').setValue("read");
    await applySettings(dialog);
    expect(wrapper.props("modelValue")).toEqual({ type: "string", deprecated: true, nullable: true, readOnly: true });
    const reopened = await openSettings(wrapper);
    await reopened.get('button[aria-label="读写行为"] + select').setValue("write");
    expect(reopened.get('input[name="schema-deprecated-0-Value"]').element).toHaveProperty("checked", true);
    await applySettings(reopened);
    expect(wrapper.props("modelValue")).toEqual({ type: "string", deprecated: true, nullable: true, writeOnly: true });
    wrapper.unmount();
  });

  it("preserves OpenAPI 3.1 null unions and edits numeric exclusive bounds", async () => {
    const wrapper = setup({ type: ["number", "null"], minimum: 0, maximum: 10 }, { openapi: "3.1.0" });
    expect(wrapper.find<HTMLSelectElement>('select[name="schema-type-0-Value"]').element.value).toBe("number");
    const dialog = await openSettings(wrapper);
    await dialog.get('input[name="schema-exclusiveMinimum-0-Value"]').setValue(true);
    await dialog.get('input[name="schema-minimum-0-Value"]').setValue("2.5");
    await applySettings(dialog);
    expect(wrapper.props("modelValue")).toEqual({ type: ["number", "null"], exclusiveMinimum: 2.5, maximum: 10 });
    const reopened = await openSettings(wrapper);
    await reopened.get('input[name="schema-nullable-0-Value"]').setValue(false);
    await reopened.get('input[name="schema-exclusiveMinimum-0-Value"]').setValue(false);
    await applySettings(reopened);
    expect(wrapper.props("modelValue")).toEqual({ type: "number", minimum: 2.5, maximum: 10 });
    wrapper.unmount();
  });

  it("normalizes incompatible constraints when changing a schema type", async () => {
    const wrapper = setup({
      type: "object",
      properties: { name: { type: "string" } },
      required: ["name"],
      additionalProperties: false,
      minProperties: 1,
      minimum: 1,
      minLength: 2,
      format: "custom",
      enum: ["name"],
      default: "name",
    });

    await wrapper.find('select[name="schema-type-0-Value"]').setValue("string");

    expect(wrapper.props("modelValue")).toEqual({ type: "string", minLength: 2, format: "custom", enum: ["name"], default: "name" });
    wrapper.unmount();
  });

  it("shows nullable property types in the primitive selector and preserves null when changing type", async () => {
    const wrapper = setup({ type: "object", properties: { nickname: { type: ["string", "null"], maxLength: 24 } } }, { openapi: "3.1.0" });
    const propertyType = wrapper.get<HTMLSelectElement>('select[name="schema-property-type-0-nickname"]');

    expect(propertyType.element.value).toBe("string");
    await propertyType.setValue("integer");

    expect(wrapper.props("modelValue")).toEqual({ type: "object", properties: { nickname: { type: ["integer", "null"] } } });
    wrapper.unmount();
  });

  it("edits a referenced allOf branch locally and preserves other references", async () => {
    const reference = { $ref: "#/components/schemas/Name" };
    const document = { openapi: "3.0.3", components: { schemas: { Name: { type: "object", properties: { name: { type: "string" } } } } } };
    const wrapper = setup({ allOf: [reference, { type: "object", properties: { id: { type: "integer" } } }], "x-owner": "contract" }, document);
    await wrapper.find('textarea[name="schema-description-2-name"]').setValue("Only this operation");
    expect(wrapper.props("modelValue")).toEqual({
      allOf: [
        { type: "object", properties: { name: { type: "string", description: "Only this operation" } } },
        { type: "object", properties: { id: { type: "integer" } } },
      ],
      "x-owner": "contract",
    });
    expect(document.components.schemas.Name.properties.name).toEqual({ type: "string" });
    wrapper.unmount();
  });

  it("adds and removes composition branches without flattening their semantics", async () => {
    const wrapper = setup({ type: "object", properties: {} });
    await wrapper.find('select[name="schema-composition-0-Value"]').setValue("oneOf");
    expect(wrapper.props("modelValue")).toMatchObject({ oneOf: [{ type: "object", properties: {} }] });
    await wrapper.find(".schema-design-composition > .schema-design-section-heading button").trigger("click");
    expect((wrapper.props("modelValue").oneOf as unknown[]).length).toBe(2);
    await wrapper.find('[aria-label="删除分支 1"]').trigger("click");
    await wrapper.find('[aria-label="删除分支 1"]').trigger("click");
    expect(wrapper.props("modelValue")).toEqual({ type: "object", properties: {} });
    wrapper.unmount();
  });

  it("edits additional property schemas and stops recursive reference expansion", async () => {
    const wrapper = setup({ type: "object", properties: {} });
    await wrapper.find('select[name="schema-additionalProperties-0-Value"]').setValue("schema");
    await wrapper.find('select[name="schema-type-1-动态字段值"]').setValue("integer");
    expect(wrapper.props("modelValue")).toMatchObject({ additionalProperties: { type: "integer" } });
    wrapper.unmount();

    const recursive = setup({ $ref: "#/components/schemas/Node" }, { openapi: "3.0.3", components: { schemas: { Node: { type: "object", properties: { next: { $ref: "#/components/schemas/Node" } } } } } });
    expect(recursive.findAll(".schema-design-node").length).toBeLessThan(4);
    expect(recursive.text()).toContain("引用字段为只读：#/components/schemas/Node");
    recursive.unmount();
  });

  it("explicitly detaches a reference while retaining intersecting 3.1 sibling rules", async () => {
    const model = { type: "string", minLength: 3 };
    const document = { openapi: "3.1.0", components: { schemas: { Name: model } } };
    const wrapper = setup({ $ref: "#/components/schemas/Name", maxLength: 10 }, document);
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "展开为独立结构")!
      .trigger("click");
    expect(wrapper.props("modelValue")).toEqual({ allOf: [model, { maxLength: 10 }] });
    expect(document.components.schemas.Name).toEqual({ type: "string", minLength: 3 });
    expect(wrapper.find(".schema-design-reference-source").exists()).toBe(false);
    wrapper.unmount();
  });
});
