import { getOpenApiOperations, isRecord, parseOpenApiContent, type OpenApiObject } from "../contract/openapi";
import type { ResponseExampleEdits } from "./openapiResponseExamples";

export type ParameterDraft = {
  key: string;
  source: "path" | "operation";
  index: number;
  name: string;
  location: string;
  originalName?: string;
  originalLocation?: string;
  type: string;
  originalType?: string;
  schema?: OpenApiObject;
  style: string;
  explode: string;
  allowReserved: boolean;
  required: boolean;
  deprecated: boolean;
  example: string;
  description: string;
  editable: boolean;
  isNew: boolean;
  fromTemplate?: boolean;
};

export type ResponseDraft = {
  key: string;
  status: string;
  statusInput: string;
  path: string[];
  name?: string;
  description: string;
  mediaTypes: string[];
  noContent?: boolean;
  mediaTypeInput: string;
  exampleEdits?: ResponseExampleEdits;
  headers?: ResponseHeaderDraft[];
  editable: boolean;
  isNew: boolean;
};

export type ResponseHeaderDraft = { key: string; name: string; definition: OpenApiObject; editable: boolean };

export type SchemaDraft = {
  id: string;
  label: string;
  mediaType: string;
  path: string[] | null;
  schema: OpenApiObject;
  shared: boolean;
  originalSchema?: string;
  responseStatus?: string;
  reference?: string;
  readonlyReason?: "external" | "missing" | "circular" | "composition";
};

export type OpenApiDesignState = {
  method: string;
  path: string;
  summary: string;
  description: string;
  operationId: string;
  tags: string;
  parameters: ParameterDraft[];
  removedOperationParameterIndexes: number[];
  requestBodyDescription: string;
  requestBodyRequired: boolean;
  requestBodyEditable: boolean;
  requestBodyPresent: boolean;
  requestBodyNew: boolean;
  requestMediaTypes: string[];
  requestMediaTypeInput: string;
  requestExampleEdits?: ResponseExampleEdits;
  responses: ResponseDraft[];
  schemaDrafts: SchemaDraft[];
  removedResponseStatuses: string[];
};

export type OpenApiDesignDraft = { baseContext: string; baseContent: string; state: OpenApiDesignState };

/** Changes to sibling operations can rebase; changes to this API or its models require review. */
export function openApiDesignContext(content: string, operationKey: string): string {
  const document = parseOpenApiContent(content);
  const entry = getOpenApiOperations(content).find((item) => item.identityKey === operationKey);
  const pathItem = entry && isRecord(document?.paths) ? document.paths[entry.path] : undefined;
  return JSON.stringify({
    openapi: document?.openapi,
    operation: entry?.operation,
    parameters: isRecord(pathItem) ? pathItem.parameters : undefined,
    components: document?.components,
    definitions: document?.definitions,
  });
}

export function openApiDesignStorageKey(scope: string, operationKey: string): string {
  return `specx-openapi-design-draft-v1:${JSON.stringify([scope, operationKey])}`;
}

function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function fields(value: Record<string, unknown>, type: "string" | "boolean", names: string[]): boolean {
  return names.every((name) => typeof value[name] === type);
}

function exampleEdits(value: unknown) {
  return value === undefined || (isRecord(value) && Object.values(value).every((edits) => isRecord(edits) && Object.values(edits).every((body) => body === null || typeof body === "string")));
}

function validState(value: unknown): value is OpenApiDesignState {
  if (!isRecord(value) || !fields(value, "string", ["method", "path", "summary", "description", "operationId", "tags", "requestBodyDescription", "requestMediaTypeInput"]) || !fields(value, "boolean", ["requestBodyRequired", "requestBodyEditable", "requestBodyPresent", "requestBodyNew"]))
    return false;
  if (!exampleEdits(value.requestExampleEdits)) return false;
  if (!strings(value.requestMediaTypes) || !strings(value.removedResponseStatuses) || !Array.isArray(value.removedOperationParameterIndexes) || !value.removedOperationParameterIndexes.every((item) => Number.isSafeInteger(item) && item >= 0)) return false;
  if (
    !Array.isArray(value.parameters) ||
    !value.parameters.every(
      (item) =>
        isRecord(item) &&
        fields(item, "string", ["key", "name", "location", "type", "style", "explode", "description"]) &&
        (item.example === undefined || typeof item.example === "string") &&
        fields(item, "boolean", ["allowReserved", "required", "deprecated", "editable", "isNew"]) &&
        ["path", "operation"].includes(String(item.source)) &&
        Number.isSafeInteger(item.index) &&
        (item.fromTemplate === undefined || typeof item.fromTemplate === "boolean") &&
        (item.schema === undefined || isRecord(item.schema)) &&
        ["originalName", "originalLocation", "originalType"].every((key) => item[key] === undefined || typeof item[key] === "string"),
    )
  )
    return false;
  if (
    !Array.isArray(value.responses) ||
    !value.responses.every(
      (item) =>
        isRecord(item) &&
        fields(item, "string", ["key", "status", "statusInput", "description", "mediaTypeInput"]) &&
        fields(item, "boolean", ["editable", "isNew"]) &&
        strings(item.path) &&
        (item.name === undefined || typeof item.name === "string") &&
        strings(item.mediaTypes) &&
        (item.noContent === undefined || typeof item.noContent === "boolean") &&
        exampleEdits(item.exampleEdits) &&
        (item.headers === undefined || (Array.isArray(item.headers) && item.headers.every((header) => isRecord(header) && fields(header, "string", ["key", "name"]) && isRecord(header.definition) && typeof header.editable === "boolean"))),
    )
  )
    return false;
  return (
    Array.isArray(value.schemaDrafts) &&
    value.schemaDrafts.every(
      (item) =>
        isRecord(item) &&
        fields(item, "string", ["id", "label", "mediaType"]) &&
        typeof item.shared === "boolean" &&
        isRecord(item.schema) &&
        (item.path === null || strings(item.path)) &&
        ["originalSchema", "responseStatus", "reference"].every((key) => item[key] === undefined || typeof item[key] === "string") &&
        (item.readonlyReason === undefined || ["external", "missing", "circular", "composition"].includes(String(item.readonlyReason))),
    )
  );
}

export function readOpenApiDesignDraft(key: string, operationKey: string): OpenApiDesignDraft | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    const value: unknown = JSON.parse(raw);
    if (isRecord(value) && typeof value.baseContent === "string" && isRecord(value.state)) {
      // Earlier device drafts predate address editing. Preserve those changes.
      const entry = getOpenApiOperations(value.baseContent).find((item) => item.identityKey === operationKey);
      value.state.method ??= entry?.method;
      value.state.path ??= entry?.path;
      if (Array.isArray(value.state.parameters)) {
        for (const parameter of value.state.parameters) {
          if (isRecord(parameter)) {
            if (parameter.example === undefined) parameter.example = "";
            parameter.deprecated ??= false;
          }
        }
      }
    }
    return isRecord(value) && typeof value.baseContext === "string" && typeof value.baseContent === "string" && validState(value.state) ? { baseContext: value.baseContext, baseContent: value.baseContent, state: value.state } : null;
  } catch {
    return null;
  }
}

export function writeOpenApiDesignDraft(key: string, draft: OpenApiDesignDraft | null): boolean {
  try {
    if (draft) localStorage.setItem(key, JSON.stringify(draft));
    else localStorage.removeItem(key);
    return true;
  } catch {
    return false;
  }
}
