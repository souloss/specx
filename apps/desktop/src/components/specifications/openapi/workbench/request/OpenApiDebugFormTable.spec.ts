import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OpenApiDebugFormTable from "./OpenApiDebugFormTable.vue";
import type { DebugFormRow } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";

describe("Body form fields", () => {
  it("edits referenced object fields and switches between scalar and upload inputs", async () => {
    const wrapper = mount(OpenApiDebugFormTable, {
      props: {
        multipart: true,
        document: { components: { schemas: { Filter: { type: "object", properties: { active: { type: "boolean" } } } } } },
        modelValue: [{ name: "filter", value: "", enabled: true }],
        "onUpdate:modelValue": (rows: DebugFormRow[]) => void wrapper.setProps({ modelValue: rows }),
      },
    });
    const select = wrapper.get('[name="openapi-form-type-0"]');
    expect(select.findAll("option").map((item) => item.attributes("value"))).toContain("file");
    await select.setValue("#/components/schemas/Filter");
    await wrapper.get(".openapi-parameter-add-value").trigger("click");
    await wrapper.get('[name="openapi-form-value-0-value-active"]').setValue("true");
    expect(wrapper.props("modelValue")[0]?.value).toBe('{"active":true}');
    await select.setValue("file");
    expect(wrapper.get<HTMLInputElement>('[name="openapi-form-file-0"]').element.type).toBe("file");
    expect(wrapper.props("modelValue")[0]).toMatchObject({ isFile: true, value: "", files: [] });
    await select.setValue("integer");
    await wrapper.get('[name="openapi-form-value-0"]').setValue("42");
    expect(wrapper.props("modelValue")[0]).toMatchObject({ type: "integer", isFile: false, value: "42" });
    wrapper.unmount();
  });
});
