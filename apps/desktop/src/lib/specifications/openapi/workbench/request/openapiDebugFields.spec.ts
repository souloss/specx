import { describe, expect, it } from "vitest";
import { debugFieldSchema, debugFieldType, reconcileFormBulkRows } from "./openapiDebugFields";

describe("debug field schemas and form bulk edits", () => {
  const document = { components: { schemas: { Filter: { type: "object", properties: { active: { type: "boolean" } } } } } };
  it("resolves selected model references and clears constraints when changing a type", () => {
    expect(debugFieldSchema("#/components/schemas/Filter", document)).toMatchObject({ type: "object", properties: { active: { type: "boolean" } } });
    expect(debugFieldType({ $ref: "#/components/schemas/Filter" })).toBe("#/components/schemas/Filter");
    expect(debugFieldSchema("integer", document)).toEqual({ type: "integer" });
  });
  it("keeps selected uploads and schema metadata through bulk edits and clears files on a type change", () => {
    const file = new File(["data"], "test.txt");
    const source = [{ name: "attachment", enabled: true, value: "", type: "file", isFile: true, files: [file], required: true }];
    const preserved = reconcileFormBulkRows(source, [{ name: "attachment", enabled: false, value: "", type: "file", description: "upload" }], true);
    expect(preserved[0]).toMatchObject({ enabled: false, files: [file], required: true, isFile: true });
    expect(reconcileFormBulkRows(source, [{ name: "attachment", enabled: true, value: "text", type: "string" }], true)[0]).toMatchObject({ value: "text", isFile: false, files: [] });
  });
});
