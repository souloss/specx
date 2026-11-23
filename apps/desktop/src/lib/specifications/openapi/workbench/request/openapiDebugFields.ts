import { isRecord, mergeOpenApiSchema, type OpenApiObject } from "../../contract/openapi";
import type { WorkbenchRow } from "../openapiWorkbench";

export type DebugFormRow = WorkbenchRow & { isFile?: boolean; files?: File[]; multiple?: boolean; required?: boolean; uploadName?: string; contentType?: string; schema?: OpenApiObject };

export function debugFieldType(schema: OpenApiObject): string {
  return typeof schema.$ref === "string" ? schema.$ref : typeof schema.type === "string" ? schema.type : isRecord(schema.properties) ? "object" : "string";
}

/** Switching types starts a fresh schema, so old enums/items cannot constrain the new editor. */
export function debugFieldSchema(type: string, document: OpenApiObject | null = null): OpenApiObject {
  const schema = type.startsWith("#/") ? { $ref: type } : type === "file" ? { type: "string", format: "binary" } : { type };
  return mergeOpenApiSchema(document, schema) ?? schema;
}

/** Match duplicate names in order, retaining File objects that text bulk editing cannot recreate. */
export function reconcileFormBulkRows(previous: DebugFormRow[], edited: WorkbenchRow[], multipart: boolean): DebugFormRow[] {
  const available = [...previous];
  return edited.map((row) => {
    const index = available.findIndex((candidate) => candidate.name === row.name);
    const old = index < 0 ? undefined : available.splice(index, 1)[0];
    const type = row.type || old?.type || (old?.isFile ? "file" : "string");
    const isFile = multipart && type === "file";
    return { ...old, ...row, type, isFile, files: isFile && old?.isFile ? old.files : [], ...(old && (old.type || (old.isFile ? "file" : "string")) !== type ? { schema: undefined } : {}) };
  });
}
