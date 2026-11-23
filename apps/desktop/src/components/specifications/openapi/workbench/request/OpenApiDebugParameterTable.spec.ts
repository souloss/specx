import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDebugParameterTable from "./OpenApiDebugParameterTable.vue";
import type { WorkbenchRow } from "@/lib/specifications/openapi/workbench/openapiWorkbench";

describe("debug parameter inputs", () => {
  it("adds, edits, disables and removes a custom query without changing declared fields", async () => {
    const wrapper = mount(OpenApiDebugParameterTable, {
      props: {
        section: "query",
        parameters: [{ key: "query:limit", name: "limit", type: "integer", description: "Limit", schema: { type: "integer" }, placeholder: "10", value: "3", enabled: true, required: false }],
        rows: [],
        "onUpdate:rows": (rows: WorkbenchRow[]) => void wrapper.setProps({ rows }),
      },
    });
    await wrapper.get(".debug-parameter-add-row input").setValue("filter");
    await wrapper.get('[name="openapi-query-name-0"]').setValue("filter");
    await wrapper.get('[name="openapi-query-value-0"]').setValue("active");
    await wrapper.get('[name="openapi-query-enabled-0"]').setValue(false);
    expect(wrapper.props("rows")).toEqual([{ name: "filter", value: "active", enabled: false }]);
    expect(wrapper.get<HTMLInputElement>("thead input").element.indeterminate).toBe(true);
    expect(wrapper.get<HTMLInputElement>('[name="openapi-query:limit"]').element.value).toBe("3");
    await wrapper.get("thead input").setValue(true);
    expect(wrapper.emitted("enabled")?.at(-1)).toEqual(["query:limit", true]);
    expect(wrapper.get<HTMLInputElement>("thead input").element.indeterminate).toBe(false);
    await wrapper.get('.debug-custom-parameter button[aria-label="删除"]').trigger("click");
    expect(wrapper.props("rows")).toEqual([]);
    wrapper.unmount();
  });

  it("uses a referenced schema for custom query values", async () => {
    const wrapper = mount(OpenApiDebugParameterTable, {
      props: {
        section: "query",
        document: { components: { schemas: { Filter: { type: "object", properties: { active: { type: "boolean" } }, additionalProperties: false } } } },
        parameters: [],
        rows: [{ name: "filter", value: "", enabled: true }],
        "onUpdate:rows": (rows: WorkbenchRow[]) => void wrapper.setProps({ rows }),
      },
    });
    await wrapper.get('[name="openapi-query-type-0"]').setValue("#/components/schemas/Filter");
    await wrapper.get(".openapi-parameter-add-value").trigger("click");
    await wrapper.get('[name="openapi-query-value-0-value-active"]').setValue("true");
    expect(wrapper.props("rows")?.[0]).toMatchObject({ type: "#/components/schemas/Filter", value: '{"active":true}' });
    wrapper.unmount();
  });

  it("disables select-all when there are no parameters", () => {
    const wrapper = mount(OpenApiDebugParameterTable, { props: { section: "query", parameters: [], rows: [] } });
    expect(wrapper.get<HTMLInputElement>("thead input").element.disabled).toBe(true);
    wrapper.unmount();
  });

  it("limits Path types to primitives while string is selected, and keeps Path names readonly", async () => {
    const wrapper = mount(OpenApiDebugParameterTable, {
      props: { section: "path", parameters: [{ key: "path:id", name: "id", type: "string", schema: { type: "string" }, description: "", placeholder: "", value: "1", enabled: true, required: true }], rows: [] },
    });
    expect(wrapper.get<HTMLInputElement>(".debug-parameter-name input").element.readOnly).toBe(true);
    const select = wrapper.get(".debug-parameter-type select.app-select-native");
    expect(select.findAll("option").map((option) => option.attributes("value"))).toEqual(["string", "integer", "boolean", "number"]);
    await select.setValue("integer");
    expect(wrapper.emitted("metadata")?.at(-1)).toEqual(["path:id", "type", "integer"]);
    wrapper.unmount();
  });
});
