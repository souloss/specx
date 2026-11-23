<script setup lang="ts">
import OpenApiMediaTypeInput from "../shared/OpenApiMediaTypeInput.vue";
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from "vue";
import { Check, Download, Lock, Plus, RotateCcw, Save, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { formatOpenApiContent, formatOpenApiExample, generateOpenApiExample, getOpenApiOperations, isRecord, materializeOpenApiSchema, parseOpenApiContent, resolveOpenApiRef, type OpenApiOperationEntry, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { moveOpenApiOperation, operationPathParameterRenames, validateOpenApiOperationAddress, type OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import { extractPathParameterNames } from "@/lib/specifications/openapi/workbench/request/openapiPathTemplate";
import { translate } from "@/i18n";
import OpenApiSchemaDesignEditor from "./OpenApiSchemaDesignEditor.vue";
import OpenApiResponseExampleDialog from "./OpenApiResponseExampleDialog.vue";
import OpenApiDesignMetadata from "./OpenApiDesignMetadata.vue";
import OpenApiDesignAddResponseDialog from "./OpenApiDesignAddResponseDialog.vue";
import OpenApiOperationTabs from "../OpenApiOperationTabs.vue";
import OpenApiResponseStatusTabs from "../shared/OpenApiResponseStatusTabs.vue";
import OpenApiDesignParameters from "./OpenApiDesignParameters.vue";
import OpenApiDesignExamples from "./OpenApiDesignExamples.vue";
import OpenApiDesignResponseHeaders from "./OpenApiDesignResponseHeaders.vue";
import OpenApiDesignResponseFormat from "./OpenApiDesignResponseFormat.vue";
import { createResponseMediaSchema, responseFormatKind } from "@/lib/specifications/openapi/design/openapiResponseFormats";
import { changeSchemaFieldType, parseSchemaFieldValue, sameSchemaValue, schemaAllowsNull, schemaFieldType, schemaModelReference } from "@/lib/specifications/openapi/design/openapiSchemaFields";
import { openApiDesignContext, openApiDesignStorageKey, readOpenApiDesignDraft, writeOpenApiDesignDraft, type OpenApiDesignDraft, type OpenApiDesignState, type ParameterDraft, type ResponseDraft, type SchemaDraft } from "@/lib/specifications/openapi/design/openapiDesignDraft";
import {
  applyResponseExampleEdits,
  parseResponseExample,
  responseCaptureTarget,
  responseExampleRows,
  responseHasBody,
  supportsResponseExample,
  validResponseExampleName,
  type CapturedResponseExample,
  type ResponseExampleEdits,
  type ResponseExampleInput,
} from "@/lib/specifications/openapi/design/openapiResponseExamples";
import { inferResponseSchema } from "@/lib/specifications/openapi/design/openapiResponseInference";

const props = withDefaults(
  defineProps<{
    content: string;
    focusOperationKey: string;
    draftScope?: string;
    editable?: boolean;
    capturedExample?: CapturedResponseExample | null;
    saveOperation?: (content: string, change?: OpenApiOperationChange) => Promise<boolean>;
  }>(),
  { editable: false },
);
const t = translate;
const emit = defineEmits<{ captured: []; debug: [] }>();
const exampleDialog = ref<{ responseKey?: string; mediaType: string; editingName?: string; body: string }>();
const exampleNotice = ref("");
const tabId = useId();
const activeParameterTab = ref("query");
const activeResponseTab = ref("");
const addResponseOpen = ref(false);
const saving = ref(false);
const panelRoot = ref<HTMLElement | null>(null);
const saveMessage = ref("");
const saveError = ref("");
const method = ref("get");
const path = ref("");
const summary = ref("");
const description = ref("");
const operationId = ref("");
const tags = ref("");
const parameters = ref<ParameterDraft[]>([]);
const removedOperationParameterIndexes = ref<number[]>([]);
const requestBodyDescription = ref("");
const requestBodyRequired = ref(false);
const requestBodyEditable = ref(false);
const requestBodyPresent = ref(false);
const requestBodyNew = ref(false);
const requestMediaTypes = ref<string[]>([]);
const requestMediaTypeInput = ref("");
const requestExampleEdits = ref<ResponseExampleEdits>({});
const mediaTypeError = ref("");
const mediaTypeErrorField = ref("");
const responses = ref<ResponseDraft[]>([]);
const selectedResponseMedia = ref<Record<string, string>>({});
const customResponseMediaKey = ref("");
const responseDescriptionKeys = ref(new Set<string>());
const schemaDrafts = ref<SchemaDraft[]>([]);
const removedResponseStatuses = ref<string[]>([]);
const loadedDraft = ref<{ key: string; baseContext: string; baseContent: string; initialState: string; persistent: boolean } | null>(null);
const draftCache = new Map<string, OpenApiDesignDraft>();
const draftPersisted = ref(false);
let persistTimer: ReturnType<typeof setTimeout> | undefined;
const draftConflict = computed(() => Boolean(loadedDraft.value && loadedDraft.value.baseContext !== openApiDesignContext(props.content, props.focusOperationKey)));
const hasDesignDraft = computed(() => Boolean(loadedDraft.value && JSON.stringify(captureDesignState()) !== loadedDraft.value.initialState));
const responseStatusError = computed(() => {
  return responses.value.some(responseStatusInvalid) ? t("specification.openapiDesignResponseStatusInvalid") : "";
});
const canCreateRequestExample = computed(() => props.editable && (!requestBodyPresent.value || (requestBodyEditable.value && requestMediaTypes.value.some(supportsResponseExample))));

const operation = computed<OpenApiOperationEntry | undefined>(() => getOpenApiOperations(props.content).find((entry) => entry.identityKey === props.focusOperationKey));
const pathNames = computed(() => extractPathParameterNames(path.value));
const pathRenames = computed(() => operationPathParameterRenames(operation.value?.path ?? "", path.value));
const pathDisplayName = (parameter: ParameterDraft) => pathRenames.value.get(parameter.name) ?? parameter.name;
const visibleParameters = computed(() => parameters.value.filter((parameter) => parameter.location !== "path" || pathNames.value.includes(pathDisplayName(parameter))));
const parameterGroups = computed(() => [
  { location: "path", heading: t("specification.openapiPathParameters") },
  { location: "query", heading: t("specification.openapiQueryParameters") },
  { location: "header", heading: t("specification.openapiHeaders") },
  { location: "cookie", heading: t("specification.openapiCookies") },
]);
const parameterTabs = computed(() => [
  ...parameterGroups.value.map((group) => ({ value: group.location, id: `${tabId}-parameter-${group.location}`, label: group.heading, count: visibleParameters.value.filter((row) => row.location === group.location).length })),
  { value: "body", id: `${tabId}-parameter-body`, label: t("specification.openapiRequestBody"), count: requestBodyPresent.value ? 1 : undefined },
]);
const responseTabs = computed(() => responses.value.map((response) => ({ value: response.key, id: `${tabId}-response-${response.key}`, label: responseName(response), status: response.status })));
watch(
  responses,
  (items) => {
    if (!items.some((item) => item.key === activeResponseTab.value)) activeResponseTab.value = items[0]?.key ?? "";
  },
  { immediate: true },
);
const format = computed<"json" | "yaml">(() => (props.content.trimStart().startsWith("{") ? "json" : "yaml"));
const parameterValidation = computed<{ key: string; message: string } | null>(() => {
  const drafts = visibleParameters.value.filter((item) => item.source === "operation" && item.editable && (item.isNew || item.name !== item.originalName || item.location !== item.originalLocation));
  for (const draft of drafts) {
    if (!draft.name.trim()) return { key: draft.key, message: t("specification.openapiDesignParameterNameRequired") };
    if (parameters.value.some((other) => other.source === "operation" && other.key !== draft.key && other.name.trim() === draft.name.trim() && other.location === draft.location)) {
      return { key: draft.key, message: t("specification.openapiDesignParameterDuplicate", { name: draft.name, location: draft.location }) };
    }
    if (draft.location === "path" && operation.value && !pathNames.value.includes(pathDisplayName(draft))) {
      return { key: draft.key, message: t("specification.openapiDesignParameterPathMismatch", { name: pathDisplayName(draft) }) };
    }
  }
  return null;
});
const parameterValidationMessage = computed(() => parameterValidation.value?.message ?? "");
const invalidParameterKey = computed(() => parameterValidation.value?.key ?? null);
const isDirty = computed(() => {
  const next = buildContent();
  if (!next) return false;
  return JSON.stringify(parseOpenApiContent(next)) !== JSON.stringify(parseOpenApiContent(props.content));
});
const invalidResponseHeaders = computed(() => responses.value.some((response) => invalidResponseHeaderKeys(response).length > 0));

function responseStatusInvalid(response: ResponseDraft): boolean {
  const status = normalizeResponseStatus(response.statusInput);
  return !/^(?:[1-5][0-9]{2}|[1-5]XX|default)$/u.test(status) || responses.value.some((other) => other.key !== response.key && normalizeResponseStatus(other.statusInput) === status);
}

function invalidResponseHeaderKeys(response: ResponseDraft): string[] {
  if (!response.editable || !response.headers) return [];
  const editableRows = response.headers.filter((header) => header.editable);
  const nameCounts = new Map<string, number>();
  for (const header of response.headers) {
    const name = header.name.trim().toLowerCase();
    nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  }
  return editableRows
    .filter((header) => {
      const name = header.name.trim().toLowerCase();
      return !/^[!#$%&'*+.^_`|~0-9a-z-]+$/u.test(name) || (nameCounts.get(name) ?? 0) > 1;
    })
    .map((header) => header.key);
}

function clearMediaTypeError() {
  mediaTypeError.value = "";
  mediaTypeErrorField.value = "";
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function stringArray(value: unknown): string {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string").join(", ") : "";
}

function exampleValue(value: unknown): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function schemaType(value: unknown): string {
  const schema = isRecord(value) ? value : null;
  if (!schema) return "—";
  const type = schema.type;
  if (Array.isArray(type)) return type.filter((item): item is string => typeof item === "string").join(" | ") || "object";
  return typeof type === "string" ? type : schema.$ref && typeof schema.$ref === "string" ? (schema.$ref.split("/").at(-1) ?? "object") : "object";
}

function parameterSchemaType(value: unknown): string {
  const schema = isRecord(value) ? materializeOpenApiSchema(parsedDocument.value, value) : value;
  return schemaType(schema);
}

function localRefPath(value: unknown): string[] | null {
  if (!isRecord(value) || typeof value.$ref !== "string") return null;
  const reference = value.$ref;
  if (!reference.startsWith("#/")) return null;
  return reference
    .slice(2)
    .split("/")
    .map((part) => {
      let decoded = part;
      try {
        decoded = decodeURIComponent(part);
      } catch {
        // Invalid escapes remain literal and resolve as a missing path.
      }
      return decoded.replaceAll("~1", "/").replaceAll("~0", "~");
    });
}

function getAtPath(root: unknown, path: string[]): unknown {
  let current = root;
  for (const part of path) {
    if (Array.isArray(current) && /^(?:0|[1-9]\d*)$/.test(part)) current = current[Number(part)];
    else if (isRecord(current) && Object.hasOwn(current, part)) current = current[part];
    else return undefined;
  }
  return current;
}

function schemaHasComposition(schema: OpenApiObject): boolean {
  return ["not", "if", "then", "else", "$dynamicRef"].some((key) => key in schema);
}

const parsedDocument = computed(() => parseOpenApiContent(props.content));
const addressError = computed(() => (parsedDocument.value ? validateOpenApiOperationAddress(parsedDocument.value, { method: method.value, path: path.value }, props.focusOperationKey) : "missing"));
const hasValidationErrors = computed(() => Boolean(addressError.value || parameterValidationMessage.value || mediaTypeError.value || responseStatusError.value || invalidResponseHeaders.value));
const saveDisabled = computed(() => Boolean(!props.editable || saving.value || !operation.value || (!isDirty.value && !hasValidationErrors.value) || draftConflict.value));
function parameterStyles(location: string): string[] {
  return location === "path" ? ["simple", "label", "matrix"] : location === "query" ? ["form", "spaceDelimited", "pipeDelimited", "deepObject"] : location === "cookie" ? ["form"] : ["simple"];
}

function schemaReadonlyMessageKey(reason: NonNullable<SchemaDraft["readonlyReason"]>) {
  switch (reason) {
    case "external":
      return "specification.openapiDesignSchemaExternalReadonly";
    case "missing":
      return "specification.openapiDesignSchemaMissingReadonly";
    case "circular":
      return "specification.openapiDesignSchemaCircularReadonly";
    case "composition":
      return "specification.openapiDesignSchemaCompositionReadonly";
  }
}

function resolveSchemaDraft(document: OpenApiObject | null, basePath: string[], source: unknown): Omit<SchemaDraft, "id" | "label" | "mediaType" | "responseStatus"> | null {
  if (!isRecord(source)) return null;
  const initialReference = typeof source.$ref === "string" ? source.$ref : undefined;
  let path = localRefPath(source) ?? basePath;
  let shared = path[0] === "components" || path[0] === "definitions";
  const visited = new Set<string>();
  for (;;) {
    const target = getAtPath(document, path);
    if (!isRecord(target)) {
      if (typeof target === "boolean" && initialReference) return { path, schema: source, shared, reference: initialReference };
      return initialReference ? { path: null, schema: source as OpenApiObject, shared, reference: initialReference, readonlyReason: "missing" } : null;
    }
    if (typeof target.$ref !== "string") {
      const pointer = `#/${path.map((part) => part.replaceAll("~", "~0").replaceAll("/", "~1")).join("/")}`;
      return {
        path,
        schema: JSON.parse(JSON.stringify(source)) as OpenApiObject,
        shared,
        ...(initialReference || shared ? { reference: initialReference ?? pointer } : {}),
        ...(schemaHasComposition(target) ? { readonlyReason: "composition" as const } : {}),
      };
    }
    const reference = target.$ref;
    const nextPath = localRefPath(target);
    if (!nextPath) return { path: null, schema: source, shared, reference, readonlyReason: "external" };
    const pathKey = nextPath.join("/");
    if (visited.has(pathKey)) return { path: null, schema: source, shared, reference, readonlyReason: "circular" };
    visited.add(pathKey);
    path = nextPath;
    shared ||= path[0] === "components" || path[0] === "definitions";
  }
}

function addSchemaDrafts(document: OpenApiObject | null, basePath: string[], source: unknown, sectionId: string, label: string, responseStatus?: string) {
  if (!isRecord(source) || !isRecord(source.content)) return;
  for (const [mediaType, media] of Object.entries(source.content)) {
    if (!isRecord(media) || !isRecord(media.schema)) continue;
    const resolved = resolveSchemaDraft(document, [...basePath, "content", mediaType, "schema"], media.schema);
    if (!resolved) continue;
    schemaDrafts.value.push({
      id: `${sectionId}:${mediaType}`,
      label,
      mediaType,
      ...resolved,
      originalSchema: JSON.stringify(resolved.schema),
      ...(responseStatus ? { responseStatus } : {}),
    });
  }
}

function resetDraft() {
  const entry = operation.value;
  activeParameterTab.value = "query";
  method.value = entry?.method ?? "get";
  path.value = entry?.path ?? "";
  const current = entry?.operation;
  summary.value = stringValue(current?.summary);
  description.value = stringValue(current?.description);
  operationId.value = stringValue(current?.operationId);
  tags.value = stringArray(current?.tags);
  parameters.value = [];
  removedOperationParameterIndexes.value = [];
  requestBodyDescription.value = "";
  requestBodyRequired.value = false;
  requestBodyEditable.value = false;
  requestBodyPresent.value = false;
  requestBodyNew.value = false;
  requestMediaTypes.value = [];
  requestMediaTypeInput.value = "";
  requestExampleEdits.value = {};
  mediaTypeError.value = "";
  responses.value = [];
  selectedResponseMedia.value = {};
  customResponseMediaKey.value = "";
  responseDescriptionKeys.value = new Set();
  schemaDrafts.value = [];
  removedResponseStatuses.value = [];
  if (!entry) return;
  const document = parseOpenApiContent(props.content);
  const pathItem = isRecord(document?.paths) && isRecord(document.paths[entry.path]) ? (document.paths[entry.path] as OpenApiObject) : null;
  const currentRecord = current as OpenApiObject | undefined;
  const rawParameters: Array<{ raw: unknown; source: ParameterDraft["source"]; index: number }> = [
    ...(Array.isArray(pathItem?.parameters) ? pathItem.parameters.map((raw, index) => ({ raw, source: "path" as const, index })) : []),
    ...(Array.isArray(currentRecord?.parameters) ? currentRecord.parameters.map((raw, index) => ({ raw, source: "operation" as const, index })) : []),
  ];
  parameters.value = rawParameters.flatMap(({ raw, source, index }) => {
    const resolved = resolveOpenApiRef(document, raw);
    if (!resolved || typeof resolved.name !== "string" || typeof resolved.in !== "string") return [];
    const resolvedSchema = isRecord(resolved.schema) ? (JSON.parse(JSON.stringify(resolved.schema)) as OpenApiObject) : undefined;
    const draftType = (resolvedSchema && schemaModelReference(resolvedSchema)) || parameterSchemaType(resolved.schema ?? { type: resolved.type });
    return [
      {
        key: `${source}:${resolved.in}:${resolved.name}:${index}`,
        source,
        index,
        name: resolved.name,
        location: resolved.in,
        originalName: resolved.name,
        originalLocation: resolved.in,
        type: draftType,
        originalType: draftType,
        ...(resolvedSchema ? { schema: resolvedSchema } : {}),
        style: stringValue(resolved.style),
        explode: typeof resolved.explode === "boolean" ? String(resolved.explode) : "",
        allowReserved: resolved.allowReserved === true,
        required: resolved.in === "path" || resolved.required === true,
        deprecated: resolved.deprecated === true,
        example: exampleValue(isRecord(resolved.schema) ? (resolved.schema.example ?? resolved.example) : resolved.example),
        description: stringValue(resolved.description),
        editable: source === "operation" && (!isRecord(raw) || typeof raw.$ref !== "string") && !isRecord(resolved.content) && (resolved.schema === undefined || isRecord(resolved.schema)),
        isNew: false,
      },
    ];
  });
  const requestBody = isRecord(current?.requestBody) ? current.requestBody : null;
  requestBodyPresent.value = Boolean(requestBody);
  const operationPath = ["paths", entry.path, entry.method];
  const rawRequestBodyPath = localRefPath(requestBody) ?? [...operationPath, "requestBody"];
  const resolvedBody = resolveOpenApiRef(document, requestBody);
  if (resolvedBody) {
    requestBodyPresent.value = true;
    requestBodyDescription.value = stringValue(resolvedBody.description);
    requestBodyRequired.value = resolvedBody.required === true;
    requestBodyEditable.value = !requestBody?.$ref;
    requestMediaTypes.value = isRecord(resolvedBody.content) ? Object.keys(resolvedBody.content) : [];
    const bodyPath = localRefPath(requestBody) ?? rawRequestBodyPath;
    addSchemaDrafts(document, bodyPath, resolvedBody, "request", t("specification.openapiRequestBody"));
  }
  const firstPopulatedParameterTab = parameterTabs.value.find((tab) => (tab.value === "path" ? pathNames.value.length > 0 : (tab.count ?? 0) > 0));
  activeParameterTab.value = firstPopulatedParameterTab?.value ?? "query";
  if (isRecord(current?.responses)) {
    responses.value = Object.entries(current.responses).flatMap(([status, raw]) => {
      const resolved = resolveOpenApiRef(document, raw);
      if (!resolved) return [];
      const mediaTypes = isRecord(resolved.content) ? Object.keys(resolved.content) : [];
      const responsePath = localRefPath(raw) ?? [...operationPath, "responses", status];
      addSchemaDrafts(document, responsePath, resolved, `response:${status}`, `${t("specification.openapiResponseBody")} ${status}`, status);
      const headers = isRecord(resolved.headers)
        ? Object.entries(resolved.headers).map(([name, value], index) => ({ key: `header:${index}`, name, definition: isRecord(value) ? (JSON.parse(JSON.stringify(value)) as OpenApiObject) : {}, editable: isRecord(value) && !("$ref" in value) && !("content" in value) }))
        : undefined;
      return [
        {
          key: `response:${status}`,
          status,
          statusInput: status,
          path: responsePath,
          name: typeof resolved["x-apifox-name"] === "string" ? resolved["x-apifox-name"] : undefined,
          description: stringValue(resolved.description),
          mediaTypes,
          mediaTypeInput: "",
          editable: !isRecord(raw) || typeof raw.$ref !== "string",
          isNew: false,
          headers,
        },
      ];
    });
  }
  saveMessage.value = "";
  saveError.value = "";
  exampleNotice.value = "";
  exampleDialog.value = undefined;
}

function cloneDocument(content: string): OpenApiObject | null {
  const document = parseOpenApiContent(content);
  if (!document) return null;
  return JSON.parse(JSON.stringify(document)) as OpenApiObject;
}

function setOrDelete(target: OpenApiObject, key: string, value: string) {
  if (value.trim()) target[key] = value.trim();
  else delete target[key];
}

function buildContent(sourceContent = props.content): string | null {
  const document = cloneDocument(sourceContent);
  const entry = operation.value;
  if (!document || !entry || !isRecord(document.paths) || !isRecord(document.paths[entry.path])) return null;
  const pathItem = document.paths[entry.path] as OpenApiObject;
  const target = pathItem[entry.method] as OpenApiObject;
  if (!isRecord(target)) return null;
  setOrDelete(target, "summary", summary.value);
  setOrDelete(target, "description", description.value);
  setOrDelete(target, "operationId", operationId.value);
  const nextTags = tags.value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
  if (nextTags.length) target.tags = nextTags;
  else delete target.tags;

  function updateParameters(rawList: unknown[], source: ParameterDraft["source"]): unknown[] {
    const next = rawList.flatMap((raw, index) => {
      if (source === "operation" && removedOperationParameterIndexes.value.includes(index)) return [];
      const draft = parameters.value.find((item) => item.source === source && item.index === index);
      if (!draft || !draft.editable || !isRecord(raw) || typeof raw.$ref === "string") return [raw];
      const next = { ...raw };
      setOrDelete(next, "name", draft.location === "path" ? pathDisplayName(draft) : draft.name);
      setOrDelete(next, "in", draft.location);
      setOrDelete(next, "description", draft.description);
      if (draft.required) next.required = true;
      else delete next.required;
      if (draft.deprecated) next.deprecated = true;
      else delete next.deprecated;
      next.schema = parameterSchema(draft, next.schema);
      setOrDelete(next, "style", draft.style);
      if (draft.explode) next.explode = draft.explode === "true";
      else delete next.explode;
      if (draft.location === "query" && draft.allowReserved) next.allowReserved = true;
      else if (next.allowReserved !== false) delete next.allowReserved;
      return [next];
    });
    if (source === "operation") {
      for (const draft of parameters.value.filter((item) => item.source === "operation" && item.isNew)) {
        if (!draft.name.trim() || (draft.fromTemplate && !pathNames.value.includes(draft.name))) continue;
        next.push({
          name: draft.location === "path" ? pathDisplayName(draft).trim() : draft.name.trim(),
          in: draft.location,
          required: draft.location === "path" || draft.required,
          ...(draft.deprecated ? { deprecated: true } : {}),
          ...(draft.description.trim() ? { description: draft.description.trim() } : {}),
          schema: parameterSchema(draft),
          ...(draft.style ? { style: draft.style } : {}),
          ...(draft.explode ? { explode: draft.explode === "true" } : {}),
          ...(draft.location === "query" && draft.allowReserved ? { allowReserved: true } : {}),
        });
      }
    }
    return next;
  }
  if (Array.isArray(target.parameters) || parameters.value.some((item) => item.source === "operation" && item.isNew) || removedOperationParameterIndexes.value.length) {
    const nextParameters = updateParameters(Array.isArray(target.parameters) ? target.parameters : [], "operation");
    if (nextParameters.length) target.parameters = nextParameters;
    else delete target.parameters;
  }
  if (!requestBodyPresent.value) delete target.requestBody;
  const body = isRecord(target.requestBody) && typeof target.requestBody.$ref !== "string" ? target.requestBody : null;
  if (requestBodyNew.value && requestBodyPresent.value) {
    target.requestBody = {
      ...(requestBodyDescription.value.trim() ? { description: requestBodyDescription.value.trim() } : {}),
      ...(requestBodyRequired.value ? { required: true } : {}),
      content: {},
    };
  } else if (body && requestBodyEditable.value) {
    setOrDelete(body, "description", requestBodyDescription.value);
    if (requestBodyRequired.value) body.required = true;
    else delete body.required;
    const content = isRecord(body.content) ? { ...body.content } : {};
    for (const mediaType of Object.keys(content)) if (!requestMediaTypes.value.includes(mediaType)) delete content[mediaType];
    if (Object.keys(content).length) body.content = content;
    else delete body.content;
  }
  if (isRecord(target.responses) || responses.value.some((draft) => draft.isNew)) {
    const nextResponses: OpenApiObject = isRecord(target.responses) ? { ...target.responses } : {};
    for (const status of removedResponseStatuses.value) delete nextResponses[status];
    for (const draft of responses.value) if (!draft.isNew && draft.path.at(-1) !== draft.status && draft.editable) delete nextResponses[draft.path.at(-1)!];
    for (const draft of responses.value) {
      if (!draft.editable) continue;
      const response = draft.isNew ? ({ description: "" } as OpenApiObject) : getAtPath(document, draft.path);
      if (!isRecord(response)) continue;
      response.description = draft.description;
      if (draft.name !== undefined) response["x-apifox-name"] = draft.name;
      const content = isRecord(response.content) ? { ...response.content } : {};
      for (const mediaType of Object.keys(content)) if (!draft.mediaTypes.includes(mediaType)) delete content[mediaType];
      for (const mediaType of draft.mediaTypes) if (!Object.hasOwn(content, mediaType)) content[mediaType] = {};
      if (Object.keys(content).length) response.content = content;
      else delete response.content;
      if (draft.noContent || !responseHasBody(draft.status)) delete response.content;
      if (draft.headers !== undefined) {
        if (draft.headers.length) response.headers = Object.fromEntries(draft.headers.map((header) => [header.name.trim(), header.definition]));
        else delete response.headers;
      }
      nextResponses[draft.status] = response;
    }
    if (Object.keys(nextResponses).length) target.responses = nextResponses;
    else delete target.responses;
  }
  for (const draft of schemaDrafts.value) {
    if (draft.readonlyReason) continue;
    if (draft.shared && JSON.stringify(draft.schema) === draft.originalSchema) continue;
    if (draft.responseStatus && (!responseHasBody(draft.responseStatus) || responses.value.some((response) => response.status === draft.responseStatus && response.noContent))) continue;
    // Apply every schema through its operation-local container, including
    // renamed responses and edited nested references. Never write components.
    const rawContainer = draft.responseStatus && isRecord(target.responses) ? target.responses[draft.responseStatus] : requestBodyPresent.value ? target.requestBody : null;
    const resolvedContainer = resolveOpenApiRef(document, rawContainer);
    if (!resolvedContainer) continue;
    const container = JSON.parse(JSON.stringify(resolvedContainer)) as OpenApiObject;
    const content = isRecord(container.content) ? container.content : {};
    const media = isRecord(content[draft.mediaType]) ? (content[draft.mediaType] as OpenApiObject) : {};
    container.content = { ...content, [draft.mediaType]: { ...media, schema: draft.schema } };
    if (draft.responseStatus && isRecord(target.responses)) target.responses[draft.responseStatus] = container;
    else target.requestBody = container;
  }
  if (isRecord(target.responses)) {
    for (const draft of responses.value) {
      const response = target.responses[draft.status];
      if (!draft.editable || !responseHasBody(draft.status) || !isRecord(response) || "$ref" in response || !isRecord(response.content)) continue;
      for (const [mediaType, edits] of Object.entries(draft.exampleEdits ?? {})) {
        const media = response.content[mediaType];
        if (draft.mediaTypes.includes(mediaType) && isRecord(media) && !("$ref" in media)) response.content[mediaType] = applyResponseExampleEdits(media, mediaType, edits);
      }
    }
  }
  if (isRecord(target.requestBody) && !("$ref" in target.requestBody) && isRecord(target.requestBody.content)) {
    for (const [mediaType, edits] of Object.entries(requestExampleEdits.value)) {
      const media = target.requestBody.content[mediaType];
      if (requestMediaTypes.value.includes(mediaType) && isRecord(media) && !("$ref" in media)) target.requestBody.content[mediaType] = applyResponseExampleEdits(media, mediaType, edits);
    }
  }
  if (validateOpenApiOperationAddress(document, { method: method.value, path: path.value }, props.focusOperationKey)) return null;
  const moved = moveOpenApiOperation(document, props.focusOperationKey, { method: method.value, path: path.value });
  return formatOpenApiContent(JSON.stringify(moved), format.value);
}

function addResponse(input: { status: string; description: string; mediaType: string }) {
  if (!props.editable || responses.value.some((response) => response.status === input.status)) return;
  const path = ["paths", operation.value?.path ?? "", operation.value?.method ?? "get", "responses", input.status];
  const key = `new-response:${crypto.randomUUID()}`;
  removedResponseStatuses.value = removedResponseStatuses.value.filter((status) => status !== input.status);
  responses.value = [...responses.value, { key, status: input.status, statusInput: input.status, path, description: input.description, mediaTypes: input.mediaType ? [input.mediaType] : [], mediaTypeInput: "", editable: true, isNew: true }];
  if (input.mediaType && responseHasBody(input.status)) {
    schemaDrafts.value = [
      ...schemaDrafts.value,
      {
        id: `response:${input.status}:${input.mediaType}`,
        label: `${t("specification.openapiResponseBody")} ${input.status}`,
        mediaType: input.mediaType,
        path: null,
        schema: createResponseMediaSchema(input.mediaType),
        shared: false,
        responseStatus: input.status,
      },
    ];
  }
  activeResponseTab.value = key;
}

function normalizedMediaType(value: string): string | null {
  const mediaType = value.trim().toLowerCase();
  return /^[a-z0-9!#$&^_.+*-]+\/[a-z0-9!#$&^_.+*-]+$/u.test(mediaType) ? mediaType : null;
}

function createMediaSchema(mediaType: string): OpenApiObject {
  return mediaType.includes("json") ? { type: "object", properties: {} } : { type: "string" };
}

function normalizeResponseStatus(value: string) {
  return value.trim().toLowerCase() === "default" ? "default" : value.trim().toUpperCase();
}

function changeResponseStatus(response: ResponseDraft, input: HTMLInputElement) {
  if (!props.editable || !response.editable) return;
  const status = normalizeResponseStatus(input.value);
  if (!/^(?:[1-5][0-9]{2}|[1-5]XX|default)$/u.test(status) || responses.value.some((other) => other.key !== response.key && other.status === status)) {
    return;
  }
  const previous = response.status;
  response.status = status;
  response.statusInput = status;
  input.value = status;
  schemaDrafts.value = schemaDrafts.value.flatMap((draft) => (draft.responseStatus !== previous ? [draft] : responseHasBody(status) ? [{ ...draft, id: `response:${status}:${draft.mediaType}`, label: `${t("specification.openapiResponseBody")} ${status}`, responseStatus: status }] : []));
  if (!responseHasBody(status)) {
    response.mediaTypes = [];
    delete response.exampleEdits;
  }
  if (mediaTypeErrorField.value === `response:${response.key}`) clearMediaTypeError();
}

function addRequestMediaType() {
  if (!requestBodyEditable.value) return;
  const mediaType = normalizedMediaType(requestMediaTypeInput.value);
  if (!mediaType || requestMediaTypes.value.some((item) => item.toLowerCase() === mediaType)) {
    mediaTypeError.value = t("specification.openapiDesignMediaTypeInvalid");
    mediaTypeErrorField.value = "request";
    return;
  }
  requestMediaTypes.value = [...requestMediaTypes.value, mediaType];
  schemaDrafts.value = [...schemaDrafts.value, { id: `request:${mediaType}`, label: t("specification.openapiRequestBody"), mediaType, path: null, schema: createMediaSchema(mediaType), shared: false }];
  requestMediaTypeInput.value = "";
  clearMediaTypeError();
}

function removeRequestMediaType(mediaType: string) {
  if (!requestBodyEditable.value || requestMediaTypes.value.length <= 1) return;
  requestMediaTypes.value = requestMediaTypes.value.filter((item) => item !== mediaType);
  delete requestExampleEdits.value[mediaType];
  schemaDrafts.value = schemaDrafts.value.filter((draft) => draft.responseStatus || draft.mediaType !== mediaType);
}

function addResponseMediaType(response: ResponseDraft) {
  if (!response.editable || !responseHasBody(response.status)) return;
  const mediaType = normalizedMediaType(response.mediaTypeInput);
  if (!mediaType || response.mediaTypes.some((item) => item.toLowerCase() === mediaType)) {
    mediaTypeError.value = t("specification.openapiDesignMediaTypeInvalid");
    mediaTypeErrorField.value = `response:${response.key}`;
    return;
  }
  response.mediaTypes = [...response.mediaTypes, mediaType];
  delete response.noContent;
  defineResponseSchema(response.status, mediaType, createResponseMediaSchema(mediaType));
  selectedResponseMedia.value[response.key] = mediaType;
  customResponseMediaKey.value = "";
  response.mediaTypeInput = "";
  clearMediaTypeError();
}

function responseName(response: ResponseDraft) {
  return response.name ?? response.description.split("\n", 1)[0] ?? "";
}

function updateResponseName(response: ResponseDraft, value: string) {
  response.name = value;
  if (!response.description && response.isNew) response.description = value;
}

function updateResponseDescription(response: ResponseDraft, value: string) {
  response.name ??= responseName(response);
  response.description = value;
}

function activeResponseMedia(response: ResponseDraft) {
  if (response.noContent || !responseHasBody(response.status)) return "";
  const selected = selectedResponseMedia.value[response.key];
  return selected && response.mediaTypes.includes(selected) ? selected : (response.mediaTypes[0] ?? "");
}

function selectResponseFormat(response: ResponseDraft, mediaType: string) {
  if (mediaType === "__custom") {
    customResponseMediaKey.value = response.key;
    return;
  }
  customResponseMediaKey.value = "";
  if (!mediaType) {
    if (response.mediaTypes.length) response.noContent = true;
    clearMediaTypeError();
  } else if (response.mediaTypes.includes(mediaType)) {
    delete response.noContent;
    selectedResponseMedia.value[response.key] = mediaType;
    clearMediaTypeError();
  } else {
    response.mediaTypeInput = mediaType;
    addResponseMediaType(response);
  }
}

function responseBodyExample(response: ResponseDraft, mediaType: string) {
  return examplesFor(response, mediaType).find((example) => example.name === "")?.body ?? "";
}

function setResponseBodyExample(response: ResponseDraft, mediaType: string, body: string) {
  setExample(response, mediaType, "", body === "" ? null : body);
}

function removeResponseMediaType(response: ResponseDraft, mediaType: string) {
  if (!response.editable) return;
  response.mediaTypes = response.mediaTypes.filter((item) => item !== mediaType);
  if (response.exampleEdits) delete response.exampleEdits[mediaType];
  schemaDrafts.value = schemaDrafts.value.filter((draft) => draft.responseStatus !== response.status || draft.mediaType !== mediaType);
}

function exampleMedia(response: ResponseDraft, mediaType: string) {
  const original = getAtPath(parsedDocument.value, [...response.path, "content", mediaType]);
  return isRecord(original) ? original : undefined;
}

function examplesFor(response: ResponseDraft, mediaType: string) {
  return responseExampleRows(exampleMedia(response, mediaType), mediaType, response.exampleEdits?.[mediaType]);
}

function canDefineResponseSchema(response: ResponseDraft, mediaType: string) {
  const media = exampleMedia(response, mediaType);
  return props.editable && !draftConflict.value && response.editable && responseHasBody(response.status) && (!media || !("schema" in media || "$ref" in media)) && !schemaDrafts.value.some((draft) => draft.responseStatus === response.status && draft.mediaType === mediaType);
}

function defineResponseSchema(status: string, mediaType: string, schema: OpenApiObject) {
  schemaDrafts.value.push({ id: `response:${status}:${mediaType}`, label: `${t("specification.openapiResponseBody")} ${status}`, mediaType, path: null, schema, shared: false, responseStatus: status });
}

function canEditExample(response: ResponseDraft, mediaType: string) {
  const original = exampleMedia(response, mediaType);
  return props.editable && !draftConflict.value && response.editable && responseHasBody(response.status) && response.mediaTypes.includes(mediaType) && supportsResponseExample(mediaType) && (!original || !("$ref" in original));
}

const exampleResponse = computed(() => responses.value.find((response) => response.key === exampleDialog.value?.responseKey));

function setExample(response: ResponseDraft, mediaType: string, name: string, body: string | null) {
  if (!canEditExample(response, mediaType)) return;
  response.exampleEdits = { ...response.exampleEdits, [mediaType]: { ...response.exampleEdits?.[mediaType], [name]: body } };
}

function applyExample(input: ResponseExampleInput) {
  const dialog = exampleDialog.value;
  if (dialog && exampleResponse.value) setExample(exampleResponse.value, dialog.mediaType, input.name, input.body);
  else if (dialog && !dialog.responseKey) setRequestExample(dialog.mediaType, input.name, input.body);
  exampleDialog.value = undefined;
}

function requestExampleMedia(mediaType: string) {
  const body = resolveOpenApiRef(parsedDocument.value, operation.value?.operation.requestBody);
  const media = isRecord(body?.content) ? body.content[mediaType] : undefined;
  return isRecord(media) ? media : undefined;
}

function setRequestExample(mediaType: string, name: string, body: string | null) {
  if (canEditRequestExample(mediaType)) {
    requestExampleEdits.value[mediaType] = { ...requestExampleEdits.value[mediaType], [name]: body };
  }
}

function canEditRequestExample(mediaType: string) {
  return props.editable && requestBodyEditable.value && requestMediaTypes.value.includes(mediaType) && !requestExampleMedia(mediaType)?.$ref;
}

function hasExampleSchema(mediaType: string, status?: string) {
  return schemaDrafts.value.some((draft) => draft.mediaType === mediaType && draft.responseStatus === status && !draft.readonlyReason);
}

const dialogNames = computed(() => {
  const dialog = exampleDialog.value;
  if (!dialog) return [];
  const rows = exampleResponse.value ? examplesFor(exampleResponse.value, dialog.mediaType) : responseExampleRows(requestExampleMedia(dialog.mediaType), dialog.mediaType, requestExampleEdits.value[dialog.mediaType]);
  return rows.map((row) => row.name);
});

function openExample(mediaType: string, response?: ResponseDraft, row?: { name: string; body: string | null }, generate = false) {
  const schema = schemaDrafts.value.find((draft) => draft.mediaType === mediaType && draft.responseStatus === response?.status)?.schema;
  const body = generate && schema ? formatOpenApiExample(generateOpenApiExample(parsedDocument.value, schema), mediaType) : (row?.body ?? "");
  exampleDialog.value = { responseKey: response?.key, mediaType, editingName: row?.name, body };
}

function applyCapturedExample(example: CapturedResponseExample): boolean {
  const entry = operation.value;
  const target = responseCaptureTarget(entry?.operation.responses, example.status, example.mediaType);
  if (!entry || !props.editable || saving.value || draftConflict.value || responseStatusError.value || target?.action !== example.action || !validResponseExampleName(example.name)) return false;
  const { status, mediaType, action } = target;
  const response = responses.value.find((item) => item.status === status);
  if (action === "newResponse") {
    if (response || responses.value.some((item) => item.path.at(-1) === status)) return false;
  } else if (!response?.editable || response.isNew || response.path.at(-1) !== status || removedResponseStatuses.value.includes(status)) return false;
  if (action === "newMedia" && response?.mediaTypes.some((key) => key.toLowerCase() === mediaType.toLowerCase())) return false;
  if (response?.noContent) return false;
  if (action === "existing" && (!response || !canEditExample(response, mediaType) || examplesFor(response, mediaType).some((row) => row.name === example.name))) return false;
  const parsed = parseResponseExample(example.body, mediaType);
  if ("error" in parsed) return false;
  let schema: OpenApiObject | null = null;
  if (example.infer) {
    if (!target.canInfer || (response && !canDefineResponseSchema(response, mediaType))) return false;
    const version = String(parsedDocument.value?.openapi);
    if (!/^3\.[01]\./.test(version)) return false;
    schema = inferResponseSchema(parsed.value, version.startsWith("3.1.") ? "3.1" : "3.0");
    if (!schema) return false;
  }
  // Commit to the local draft only after all checks pass.
  if (!response) {
    responses.value.push({
      key: `new-response:${crypto.randomUUID()}`,
      status,
      statusInput: status,
      path: ["paths", entry.path, entry.method, "responses", status],
      description: t("specification.openapiResponseExample.statusDescription", { status }),
      mediaTypes: [mediaType],
      mediaTypeInput: "",
      editable: true,
      isNew: true,
      exampleEdits: { [mediaType]: { [example.name]: example.body } },
    });
  } else {
    if (action === "newMedia") response.mediaTypes.push(mediaType);
    setExample(response, mediaType, example.name, example.body);
  }
  if (schema) defineResponseSchema(status, mediaType, schema);
  return true;
}

function addParameter(location: string, requestedName = "") {
  let index = parameters.value.filter((item) => item.isNew).length + 1;
  let name = requestedName || `parameter${index}`;
  while (parameters.value.some((item) => item.name === name)) {
    index += 1;
    name = `parameter${index}`;
  }
  const key = `new:${crypto.randomUUID()}`;
  parameters.value = [...parameters.value, { key, source: "operation", index: -1, name, location, type: "string", style: "", explode: "", allowReserved: false, required: false, deprecated: false, example: "", description: "", editable: true, isNew: true }];
  void nextTick(() => document.getElementById(`design-parameter-${key}`)?.focus());
}

function syncPathParameters() {
  if (addressError.value || !props.editable) return;
  const existing = new Set(parameters.value.filter((row) => row.location === "path").map(pathDisplayName));
  for (const name of pathNames.value) {
    if (existing.has(name)) continue;
    parameters.value.push({ key: `template:${name}`, source: "operation", index: -1, name, location: "path", type: "string", style: "", explode: "", allowReserved: false, required: true, deprecated: false, example: "", description: "", editable: true, isNew: true, fromTemplate: true });
  }
}

function removeParameter(parameter: ParameterDraft) {
  if (parameter.source === "operation" && !parameter.isNew && !removedOperationParameterIndexes.value.includes(parameter.index)) {
    removedOperationParameterIndexes.value = [...removedOperationParameterIndexes.value, parameter.index];
  }
  parameters.value = parameters.value.filter((item) => item.key !== parameter.key);
}

function setParameterLocation(parameter: ParameterDraft, location: string) {
  parameter.location = location;
  if (parameter.style && !parameterStyles(location).includes(parameter.style)) parameter.style = "";
  if (location !== "query") parameter.allowReserved = false;
  if (location === "path") parameter.required = true;
}

function parameterSchema(parameter: ParameterDraft, original?: unknown): OpenApiObject {
  const raw = parameter.source === "operation" && !parameter.isNew && Array.isArray(operation.value?.operation.parameters) ? operation.value.operation.parameters[parameter.index] : undefined;
  const resolved = resolveOpenApiRef(parsedDocument.value, raw);
  const source = original ?? resolved?.schema;
  let schema = { ...(parameter.schema ?? (isRecord(source) ? source : { type: parameter.originalType ?? parameter.type })) };
  const reference = schemaModelReference(schema);
  if (!reference && parameter.type.startsWith("#/")) return { $ref: parameter.type };
  if (!reference && parameterSchemaType(schema) !== parameter.type) schema = changeSchemaFieldType(schema, parameter.type, parsedDocument.value);
  if (!reference && parameter.example !== exampleValue(schema.example ?? resolved?.example)) {
    if (parameter.example === "") delete schema.example;
    else {
      const parsed = parseSchemaFieldValue(parameter.example, schemaFieldType(schema), schemaAllowsNull(schema));
      schema.example = parsed ? parsed.value : parameter.example;
    }
  }
  return schema;
}

function updateParameterSchema(parameter: ParameterDraft, schema: OpenApiObject) {
  if (!sameSchemaValue(parameterSchema(parameter).example, schema.example)) parameter.example = exampleValue(schema.example);
  parameter.schema = schema;
  parameter.type = schemaModelReference(schema) || parameterSchemaType(schema);
}

function addRequestBody() {
  requestBodyPresent.value = true;
  requestBodyNew.value = true;
  requestBodyEditable.value = true;
  requestBodyDescription.value = "";
  requestBodyRequired.value = false;
  requestMediaTypes.value = ["application/json"];
  schemaDrafts.value = [...schemaDrafts.value, { id: "request:application/json", label: t("specification.openapiRequestBody"), mediaType: "application/json", path: null, schema: { type: "object", properties: {} }, shared: false }];
}

function openRequestExamples() {
  if (!props.editable) return;
  activeParameterTab.value = "body";
  if (!requestBodyPresent.value) addRequestBody();
  const mediaType = requestMediaTypes.value.find((item) => supportsResponseExample(item) && canEditRequestExample(item));
  if (mediaType) openExample(mediaType, undefined, undefined, hasExampleSchema(mediaType));
}

function removeRequestBody() {
  requestBodyPresent.value = false;
  requestBodyNew.value = false;
  requestMediaTypes.value = [];
  requestExampleEdits.value = {};
  schemaDrafts.value = schemaDrafts.value.filter((draft) => draft.responseStatus);
}

function removeResponse(status: string) {
  const response = responses.value.find((item) => item.status === status);
  if (response && !response.isNew) removedResponseStatuses.value = [...new Set([...removedResponseStatuses.value, response.path.at(-1) ?? status])];
  responses.value = responses.value.filter((response) => response.status !== status);
  schemaDrafts.value = schemaDrafts.value.filter((draft) => draft.responseStatus !== status);
}

function updateSchemaDraft(id: string, schema: OpenApiObject) {
  schemaDrafts.value = schemaDrafts.value.map((draft) => (draft.id === id ? { ...draft, schema, readonlyReason: undefined } : draft));
}

function captureDesignState(): OpenApiDesignState {
  return JSON.parse(
    JSON.stringify({
      method: method.value,
      path: path.value,
      summary: summary.value,
      description: description.value,
      operationId: operationId.value,
      tags: tags.value,
      parameters: parameters.value,
      removedOperationParameterIndexes: removedOperationParameterIndexes.value,
      requestBodyDescription: requestBodyDescription.value,
      requestBodyRequired: requestBodyRequired.value,
      requestBodyEditable: requestBodyEditable.value,
      requestBodyPresent: requestBodyPresent.value,
      requestBodyNew: requestBodyNew.value,
      requestMediaTypes: requestMediaTypes.value,
      requestMediaTypeInput: requestMediaTypeInput.value,
      requestExampleEdits: requestExampleEdits.value,
      responses: responses.value,
      schemaDrafts: schemaDrafts.value,
      removedResponseStatuses: removedResponseStatuses.value,
    }),
  ) as OpenApiDesignState;
}

function applyDesignState(state: OpenApiDesignState) {
  method.value = state.method;
  path.value = state.path;
  summary.value = state.summary;
  description.value = state.description;
  operationId.value = state.operationId;
  tags.value = state.tags;
  parameters.value = state.parameters.map((parameter) => ({ ...parameter, deprecated: parameter.deprecated ?? false, example: parameter.example ?? "" }));
  removedOperationParameterIndexes.value = state.removedOperationParameterIndexes;
  requestBodyDescription.value = state.requestBodyDescription;
  requestBodyRequired.value = state.requestBodyRequired;
  requestBodyEditable.value = state.requestBodyEditable;
  requestBodyPresent.value = state.requestBodyPresent;
  requestBodyNew.value = state.requestBodyNew;
  requestMediaTypes.value = state.requestMediaTypes;
  requestMediaTypeInput.value = state.requestMediaTypeInput;
  requestExampleEdits.value = state.requestExampleEdits ?? {};
  responses.value = state.responses.map((response) => ({ ...response, headers: response.headers ?? responses.value.find((item) => item.key === response.key)?.headers }));
  schemaDrafts.value = state.schemaDrafts;
  removedResponseStatuses.value = state.removedResponseStatuses;
}

function persistDesignDraft() {
  clearTimeout(persistTimer);
  const loaded = loadedDraft.value;
  if (!loaded) return;
  const state = captureDesignState();
  const draft = JSON.stringify(state) === loaded.initialState ? null : { baseContext: loaded.baseContext, baseContent: loaded.baseContent, state };
  if (draft) draftCache.set(loaded.key, draft);
  else draftCache.delete(loaded.key);
  draftPersisted.value = loaded.persistent && writeOpenApiDesignDraft(loaded.key, draft);
}

function loadDesignDraft() {
  persistDesignDraft();
  resetDraft();
  const key = openApiDesignStorageKey(props.draftScope ?? "", props.focusOperationKey);
  const baseContext = openApiDesignContext(props.content, props.focusOperationKey);
  const initialState = JSON.stringify(captureDesignState());
  const draft = draftCache.get(key) ?? (props.draftScope ? readOpenApiDesignDraft(key, props.focusOperationKey) : null);
  loadedDraft.value = { key, baseContext, baseContent: props.content, initialState, persistent: Boolean(props.draftScope) };
  draftPersisted.value = false;
  if (!draft) return;
  // Clone so subsequent edits cannot mutate the cached draft for another API.
  applyDesignState(JSON.parse(JSON.stringify(draft.state)) as OpenApiDesignState);
  loadedDraft.value.baseContext = draft.baseContext;
  loadedDraft.value.baseContent = draft.baseContent;
  persistDesignDraft();
}

function discardDesignDraft() {
  resetDraft();
  if (loadedDraft.value) {
    loadedDraft.value.baseContext = openApiDesignContext(props.content, props.focusOperationKey);
    loadedDraft.value.baseContent = props.content;
    loadedDraft.value.initialState = JSON.stringify(captureDesignState());
  }
  persistDesignDraft();
}

function downloadDesignDraft() {
  const content = buildContent(loadedDraft.value?.baseContent);
  if (!content) return;
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([content], { type: "text/plain;charset=utf-8" }));
  link.download = `openapi-design-draft.${format.value}`;
  link.click();
  URL.revokeObjectURL(link.href);
}

async function focusFirstInvalid() {
  let targetKind: "address" | "parameter" | "request-body" | "response";
  let targetResponse: ResponseDraft | undefined;
  if (addressError.value) targetKind = "address";
  else if (parameterValidation.value) {
    const parameter = parameters.value.find((item) => item.key === parameterValidation.value?.key);
    if (parameter?.location === "path") targetKind = "address";
    else {
      targetKind = "parameter";
      activeParameterTab.value = parameter?.location ?? "query";
    }
  } else if (mediaTypeErrorField.value === "request") {
    targetKind = "request-body";
    activeParameterTab.value = "body";
  } else if (mediaTypeErrorField.value.startsWith("response:")) {
    targetKind = "response";
    targetResponse = responses.value.find((response) => response.key === mediaTypeErrorField.value.slice("response:".length));
  } else if (responseStatusError.value) {
    targetKind = "response";
    targetResponse = responses.value.find(responseStatusInvalid);
  } else {
    targetKind = "response";
    targetResponse = responses.value.find((response) => invalidResponseHeaderKeys(response).length > 0);
  }
  if (targetResponse) {
    activeResponseTab.value = targetResponse.key;
    if (mediaTypeErrorField.value === `response:${targetResponse.key}`) customResponseMediaKey.value = targetResponse.key;
  }
  await nextTick();
  const root = panelRoot.value;
  if (!root) return;
  let target: HTMLElement | null;
  if (targetKind === "address") {
    target = root.querySelector<HTMLInputElement>('input[name="openapi-design-path"]');
  } else if (targetKind === "parameter") {
    target = [...root.querySelectorAll<HTMLElement>("[data-parameter-location]")].find((group) => group.dataset.parameterLocation === activeParameterTab.value) ?? null;
  } else if (targetKind === "request-body") {
    target = root.querySelector<HTMLElement>(".openapi-design-request-body");
  } else {
    target = targetResponse ? ([...root.querySelectorAll<HTMLElement>(".openapi-design-response-row")].find((row) => row.dataset.responseStatus === targetResponse.status) ?? null) : null;
  }
  const responseHeadersAreInvalid = targetResponse ? invalidResponseHeaderKeys(targetResponse).length > 0 : false;
  const details = responseHeadersAreInvalid ? target?.querySelector<HTMLDetailsElement>(".openapi-design-response-headers") : null;
  if (details) {
    details.open = true;
    await nextTick();
  }
  target?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
}

async function save() {
  if (!props.editable || !props.saveOperation || saving.value || draftConflict.value) return;
  if (addressError.value || parameterValidationMessage.value || mediaTypeError.value || responseStatusError.value || invalidResponseHeaders.value) {
    await focusFirstInvalid();
    return;
  }
  const next = buildContent();
  if (!next) return;
  const savedKey = loadedDraft.value?.key;
  const savedState = JSON.stringify(captureDesignState());
  const change = { previousKey: props.focusOperationKey, key: `${method.value.toUpperCase()}:${path.value.trim()}`, title: `${method.value.toUpperCase()} ${path.value.trim()}` };
  saving.value = true;
  saveMessage.value = "";
  saveError.value = "";
  try {
    if (await props.saveOperation(next, change)) {
      await nextTick();
      if (loadedDraft.value?.key === savedKey && JSON.stringify(captureDesignState()) === savedState) {
        discardDesignDraft();
        saveMessage.value = t("specification.openapiDesignSaved");
      } else if (savedKey && JSON.stringify(draftCache.get(savedKey)?.state) === savedState) {
        draftCache.delete(savedKey);
        if (props.draftScope) writeOpenApiDesignDraft(savedKey, null);
      }
    } else if (loadedDraft.value?.key === savedKey) saveError.value = t("specification.openapiDesignSaveFailed");
  } catch (cause) {
    if (loadedDraft.value?.key === savedKey) saveError.value = cause instanceof Error ? cause.message : t("specification.openapiDesignSaveFailed");
  } finally {
    saving.value = false;
  }
}

watch(() => [props.content, props.focusOperationKey, props.draftScope] as const, loadDesignDraft, { immediate: true });
watch([path, addressError], syncPathParameters);
watch(
  () => props.capturedExample,
  (example) => {
    if (!example || example.operationKey !== props.focusOperationKey) return;
    if (applyCapturedExample(example)) {
      persistDesignDraft();
      exampleNotice.value = t(example.infer ? "specification.openapiResponseExample.schemaStaged" : "specification.openapiResponseExample.staged", { name: example.name });
    } else exampleNotice.value = t("specification.openapiResponseExample.notStaged");
    emit("captured");
  },
  { immediate: true, flush: "post" },
);
watch(
  () => JSON.stringify(captureDesignState()),
  () => {
    clearTimeout(persistTimer);
    draftPersisted.value = false;
    persistTimer = setTimeout(persistDesignDraft, 250);
  },
);
window.addEventListener("pagehide", persistDesignDraft);
onBeforeUnmount(() => {
  persistDesignDraft();
  window.removeEventListener("pagehide", persistDesignDraft);
});
</script>

<template>
  <section ref="panelRoot" class="openapi-design-panel" :aria-label="t('specification.openapiDesignTab')">
    <div v-if="draftConflict" class="openapi-design-conflict" role="alert">
      <strong>{{ t("specification.openapiDesignDraftConflictTitle") }}</strong>
      <p>{{ t("specification.openapiDesignDraftConflictDescription") }}</p>
      <Button size="sm" variant="outline" @click="downloadDesignDraft"><Download class="h-3.5 w-3.5" />{{ t("specification.openapiDesignDownloadDraft") }}</Button>
      <Button size="sm" variant="outline" :disabled="saving" @click="discardDesignDraft"><RotateCcw class="h-3.5 w-3.5" />{{ t("specification.openapiDesignDiscardDraft") }}</Button>
    </div>
    <p v-if="saveMessage" class="openapi-design-message success" role="status"><Check class="h-3.5 w-3.5" />{{ saveMessage }}</p>
    <p v-if="saveError" class="openapi-design-message error" role="alert">{{ saveError }}</p>
    <p v-if="exampleNotice" class="openapi-design-message" role="status">{{ exampleNotice }}</p>
    <p v-if="mediaTypeError" id="openapi-design-media-type-error" class="openapi-design-message error" role="alert">{{ mediaTypeError }}</p>
    <p v-if="responseStatusError" id="openapi-design-response-status-error" class="openapi-design-message error" role="alert">{{ responseStatusError }}</p>
    <p v-if="invalidResponseHeaders" id="openapi-design-response-header-error" class="openapi-design-message error" role="alert">{{ t("specification.openapiDesignResponseHeaderInvalid") }}</p>
    <fieldset :disabled="saving" class="openapi-design-grid openapi-design-fields">
      <OpenApiDesignMetadata v-model:method="method" v-model:path="path" v-model:summary="summary" v-model:operation-id="operationId" v-model:tags="tags" v-model:description="description" :editable="props.editable" :address-error="addressError">
        <template #actions>
          <span class="openapi-design-operation-actions">
            <span v-if="!props.editable" class="openapi-design-readonly"><Lock class="h-3.5 w-3.5" />{{ t("specification.openapiDesignReadonly") }}</span>
            <span v-if="hasDesignDraft && !draftConflict" class="openapi-design-draft-status" role="status">{{ draftPersisted ? t("specification.openapiDesignDraftLocal") : t("specification.openapiRequestUnsavedChanges") }}</span>
            <span class="openapi-design-actions"
              ><Button size="sm" :disabled="saveDisabled ? true : undefined" @click="save"><Save class="h-3.5 w-3.5" />{{ saving ? t("common.saving") : t("common.save") }}</Button></span
            >
            <Button type="button" size="sm" variant="outline" :disabled="saving" @click="emit('debug')">{{ t("specification.openapiDesignDebug") }}</Button>
            <details class="openapi-design-more-actions">
              <summary :aria-label="t('specification.openapiDesignMoreActions')" :title="t('specification.openapiDesignMoreActions')"><span aria-hidden="true">•••</span></summary>
              <div class="openapi-design-more-menu" role="group" :aria-label="t('specification.openapiDesignMoreActions')">
                <button type="button" :disabled="!hasDesignDraft && !draftConflict" @click="downloadDesignDraft"><Download class="h-3.5 w-3.5" />{{ t("specification.openapiDesignDownloadDraft") }}</button>
                <button type="button" :disabled="saving || (!hasDesignDraft && !draftConflict)" @click="discardDesignDraft"><RotateCcw class="h-3.5 w-3.5" />{{ t("common.reset") }}</button>
              </div>
            </details>
          </span>
        </template>
        <template #shortcuts>
          <button type="button" class="openapi-design-shortcut" :disabled="saving || !canCreateRequestExample" @click="openRequestExamples">＋ {{ t("specification.openapiDesignCustomRequestExample") }}</button>
        </template>
      </OpenApiDesignMetadata>

      <section class="openapi-design-card openapi-design-wide">
        <div class="openapi-design-card-heading">
          <div>
            <h4>{{ t("specification.openapiDesignRequestTitle") }}</h4>
          </div>
        </div>
        <p v-if="parameterValidationMessage" id="openapi-design-parameter-error" class="openapi-design-message error" role="alert">{{ parameterValidationMessage }}</p>
        <div class="openapi-design-request-panel">
          <OpenApiOperationTabs v-model="activeParameterTab" :tabs="parameterTabs" :ariaLabel="t('specification.openapiDesignRequestTitle')" />
          <OpenApiDesignParameters
            v-for="group in parameterGroups"
            v-show="activeParameterTab === group.location"
            role="tabpanel"
            :aria-labelledby="`${tabId}-parameter-${group.location}`"
            :key="group.location"
            :location="group.location"
            :heading="group.heading"
            :rows="visibleParameters.filter((row) => row.location === group.location)"
            :editable="props.editable"
            :parsed-document="parsedDocument"
            :path-name="pathDisplayName"
            :parameter-schema="parameterSchema"
            :parameter-styles="parameterStyles"
            :invalid-parameter-key="invalidParameterKey"
            @add="addParameter(group.location, $event)"
            @remove="removeParameter"
            @location="setParameterLocation"
            @schema="updateParameterSchema"
          />
          <div v-show="activeParameterTab === 'body'" class="openapi-design-request-body" role="tabpanel" :aria-labelledby="`${tabId}-parameter-body`">
            <Button v-if="!requestBodyPresent" size="sm" variant="outline" :disabled="!props.editable" @click="addRequestBody"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiDesignAddRequestBody") }}</Button>
            <div v-if="requestBodyPresent" class="openapi-design-subcard">
              <div class="openapi-design-subcard-heading">
                <span>{{ t("specification.openapiRequestBody") }}</span>
                <button type="button" class="icon-button" :aria-label="t('specification.openapiDesignRemoveRequestBody')" :disabled="!props.editable" @click="removeRequestBody"><Trash2 class="h-3.5 w-3.5" /></button>
                <span v-if="!requestBodyEditable" class="openapi-design-ref-lock"><Lock class="h-3 w-3" />{{ t("specification.openapiDesignReferenceReadonly") }}</span>
              </div>
              <label class="openapi-design-field"
                ><span>{{ t("specification.openapiDescription") }}</span
                ><textarea v-model="requestBodyDescription" class="field-control openapi-design-textarea" :disabled="!props.editable || !requestBodyEditable" rows="3" />
              </label>
              <label class="openapi-design-check"><input v-model="requestBodyRequired" type="checkbox" :disabled="!props.editable || !requestBodyEditable" />{{ t("specification.openapiRequired") }}</label>
            </div>
            <div v-if="requestBodyPresent" class="openapi-design-media-editor">
              <span class="openapi-design-media-label">{{ t("specification.openapiDesignMediaType") }}</span>
              <div class="openapi-design-media-list">
                <span v-for="mediaType in requestMediaTypes" :key="mediaType" class="openapi-design-media-pill">
                  <code>{{ mediaType }}</code>
                  <button v-if="requestBodyEditable" type="button" class="icon-button" :aria-label="t('specification.openapiDesignRemoveMediaType', { mediaType })" :disabled="!props.editable || requestMediaTypes.length <= 1" @click="removeRequestMediaType(mediaType)">
                    <Trash2 class="h-3 w-3" />
                  </button>
                </span>
              </div>
              <div v-if="requestBodyEditable" class="openapi-design-media-add">
                <OpenApiMediaTypeInput v-model="requestMediaTypeInput" name="openapi-request-media-type" :disabled="!props.editable" :invalid="mediaTypeErrorField === 'request'" error-id="openapi-design-media-type-error" @input="clearMediaTypeError" @submit="addRequestMediaType" />
                <Button size="xs" variant="outline" :disabled="!props.editable" @click="addRequestMediaType"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiDesignAddMediaType") }}</Button>
              </div>
            </div>
            <div v-for="draft in schemaDrafts.filter((item) => !item.responseStatus)" :key="draft.id" class="openapi-design-schema-card">
              <div class="openapi-design-schema-heading">
                <strong>{{ draft.label }}</strong
                ><code>{{ draft.mediaType }}</code>
              </div>
              <p v-if="draft.shared && typeof draft.schema.$ref !== 'string' && !draft.readonlyReason && JSON.stringify(draft.schema) === draft.originalSchema" class="openapi-design-schema-notice">{{ t("specification.openapiDesignSharedSchemaNotice", { ref: draft.reference ?? draft.label }) }}</p>
              <p v-if="draft.readonlyReason" class="openapi-design-schema-notice readonly">{{ t(schemaReadonlyMessageKey(draft.readonlyReason), { ref: draft.reference ?? "" }) }}</p>
              <OpenApiSchemaDesignEditor :model-value="draft.schema" :label="draft.label" :document="parsedDocument" :editable="props.editable" @update:model-value="updateSchemaDraft(draft.id, $event)" />
            </div>
            <template v-for="mediaType in requestMediaTypes" :key="mediaType">
              <OpenApiDesignExamples
                v-if="supportsResponseExample(mediaType)"
                request
                :media-type="mediaType"
                :rows="responseExampleRows(requestExampleMedia(mediaType), mediaType, requestExampleEdits[mediaType])"
                :editable="canEditRequestExample(mediaType)"
                :can-generate="hasExampleSchema(mediaType)"
                @add="openExample(mediaType)"
                @generate="openExample(mediaType, undefined, undefined, true)"
                @edit="openExample(mediaType, undefined, $event)"
                @remove="setRequestExample(mediaType, $event, null)"
              />
            </template>
          </div>
        </div>
      </section>

      <section class="openapi-design-card openapi-design-wide">
        <div class="openapi-design-card-heading">
          <div>
            <h4>{{ t("specification.openapiDesignResponsesTitle") }}</h4>
          </div>
          <Button v-if="!responses.length" size="sm" variant="ghost" :disabled="!props.editable" @click="addResponseOpen = true"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiDesignAddResponse") }}</Button>
        </div>
        <div v-if="responses.length" class="openapi-response-panel openapi-design-response-list">
          <div class="openapi-design-response-tabbar">
            <OpenApiResponseStatusTabs v-model="activeResponseTab" :tabs="responseTabs" :ariaLabel="t('specification.openapiDesignResponsesTitle')" />
            <Button size="xs" variant="ghost" :disabled="!props.editable" @click="addResponseOpen = true"><Plus class="size-3.5" />{{ t("specification.openapiDesignAddResponse") }}</Button>
          </div>
          <div v-for="response in responses" v-show="activeResponseTab === response.key" :key="response.key" class="openapi-design-response-row" :data-response-status="response.status" role="tabpanel" :aria-labelledby="`${tabId}-response-${response.key}`">
            <div class="openapi-design-response-fields">
              <label class="openapi-design-field openapi-design-response-status" :class="response.status.startsWith('2') ? 'success' : response.status.startsWith('4') || response.status.startsWith('5') ? 'error' : ''">
                <span>{{ t("specification.openapiDesignResponseHttpStatus") }}</span>
                <input
                  v-model="response.statusInput"
                  class="field-control font-mono"
                  :name="`openapi-response-status-${response.key}`"
                  :aria-label="t('specification.openapiDesignResponseStatus')"
                  :aria-invalid="responseStatusInvalid(response)"
                  :aria-describedby="responseStatusInvalid(response) ? 'openapi-design-response-status-error' : undefined"
                  :disabled="!props.editable || !response.editable"
                  autocomplete="off"
                  spellcheck="false"
                  @change="changeResponseStatus(response, $event.target as HTMLInputElement)"
                />
              </label>
              <label class="openapi-design-field">
                <span>{{ t("specification.openapiName") }}</span>
                <input :value="responseName(response)" class="field-control" :name="`openapi-response-name-${response.key}`" :disabled="!props.editable || !response.editable" @input="updateResponseName(response, ($event.target as HTMLInputElement).value)" />
              </label>
              <label class="openapi-design-field openapi-design-response-format">
                <span>{{ t("specification.openapiDesignResponseFormat") }}</span>
                <OpenApiDesignResponseFormat
                  :model-value="activeResponseMedia(response)"
                  :media-types="response.mediaTypes"
                  :name="`openapi-response-format-${response.key}`"
                  :editable="props.editable && response.editable"
                  :disabled="!responseHasBody(response.status) || ((!props.editable || !response.editable) && response.mediaTypes.length < 2)"
                  @update:model-value="selectResponseFormat(response, $event)"
                />
                <code v-if="activeResponseMedia(response)" :title="activeResponseMedia(response)">{{ activeResponseMedia(response) }}</code>
              </label>
              <button type="button" class="icon-button" :aria-label="t('specification.openapiDesignRemoveResponse', { status: response.status })" :disabled="!props.editable" @click="removeResponse(response.status)"><Trash2 class="size-3.5" aria-hidden="true" /></button>
            </div>
            <div v-if="customResponseMediaKey === response.key" class="openapi-design-media-add openapi-response-custom-format">
              <OpenApiMediaTypeInput
                v-model="response.mediaTypeInput"
                :name="`openapi-response-media-type-${response.status}`"
                :disabled="!props.editable"
                :invalid="mediaTypeErrorField === `response:${response.key}`"
                error-id="openapi-design-media-type-error"
                @input="clearMediaTypeError"
                @submit="addResponseMediaType(response)"
              />
              <Button size="xs" variant="outline" :disabled="!props.editable" @click="addResponseMediaType(response)"><Plus class="size-3.5" />{{ t("specification.openapiDesignAddMediaType") }}</Button>
              <Button
                size="xs"
                variant="ghost"
                @click="
                  customResponseMediaKey = '';
                  clearMediaTypeError();
                "
                >{{ t("common.cancel") }}</Button
              >
            </div>
            <p v-if="!responseHasBody(response.status)" class="openapi-design-empty">{{ t("specification.openapiDesignResponseNoBody") }}</p>
            <section v-if="responseHasBody(response.status) && activeResponseMedia(response)" :key="activeResponseMedia(response)" class="openapi-response-design-body">
              <header class="openapi-design-schema-heading">
                <strong>{{ t("specification.openapiResponseBody") }}</strong>
                <button v-if="props.editable && response.editable" type="button" class="icon-button ml-auto" :aria-label="t('specification.openapiDesignRemoveMediaType', { mediaType: activeResponseMedia(response) })" @click="removeResponseMediaType(response, activeResponseMedia(response))">
                  <Trash2 class="size-3.5" aria-hidden="true" />
                </button>
              </header>
              <div v-if="responseFormatKind(activeResponseMedia(response)) === 'text'" class="openapi-response-body-text">
                <textarea
                  class="field-control font-mono"
                  :name="`openapi-response-body-${response.key}`"
                  :value="responseBodyExample(response, activeResponseMedia(response))"
                  :aria-label="t('specification.openapiResponseExample.body')"
                  :placeholder="t('specification.openapiResponseExample.body')"
                  :readonly="!canEditExample(response, activeResponseMedia(response)) || examplesFor(response, activeResponseMedia(response)).some((item) => item.name !== '')"
                  spellcheck="false"
                  rows="5"
                  @input="setResponseBodyExample(response, activeResponseMedia(response), ($event.target as HTMLTextAreaElement).value)"
                />
              </div>
              <div v-for="draft in schemaDrafts.filter((item) => item.responseStatus === response.status && item.mediaType === activeResponseMedia(response))" :key="draft.id" class="openapi-design-schema-card">
                <code class="sr-only">{{ draft.mediaType }}</code>
                <p v-if="draft.shared && typeof draft.schema.$ref !== 'string' && !draft.readonlyReason && JSON.stringify(draft.schema) === draft.originalSchema" class="openapi-design-schema-notice">{{ t("specification.openapiDesignSharedSchemaNotice", { ref: draft.reference ?? draft.label }) }}</p>
                <p v-if="draft.readonlyReason" class="openapi-design-schema-notice readonly">{{ t(schemaReadonlyMessageKey(draft.readonlyReason), { ref: draft.reference ?? "" }) }}</p>
                <OpenApiSchemaDesignEditor v-if="responseFormatKind(draft.mediaType) !== 'text'" :model-value="draft.schema" :label="draft.label" :document="parsedDocument" :editable="props.editable" compact @update:model-value="updateSchemaDraft(draft.id, $event)" />
                <details v-else>
                  <summary>{{ t("specification.openapiSchemaAdvancedSettings") }}</summary>
                  <OpenApiSchemaDesignEditor :model-value="draft.schema" :label="draft.label" :document="parsedDocument" :editable="props.editable" @update:model-value="updateSchemaDraft(draft.id, $event)" />
                </details>
              </div>
              <Button v-if="canDefineResponseSchema(response, activeResponseMedia(response))" class="m-2" size="xs" variant="outline" @click="defineResponseSchema(response.status, activeResponseMedia(response), createResponseMediaSchema(activeResponseMedia(response)))">
                <Plus class="size-3" />{{ t("specification.openapiResponseExample.defineSchema") }}
              </Button>
            </section>
            <div v-if="!response.editable" class="openapi-design-ref-lock"><Lock class="size-3" />{{ t("specification.openapiDesignReferenceReadonly") }}</div>
            <div class="openapi-response-design-actions">
              <OpenApiDesignResponseHeaders :rows="response.headers" :document="parsedDocument" :editable="props.editable && response.editable" :invalid-keys="invalidResponseHeaderKeys(response)" @update="response.headers = $event" />
              <Button v-if="props.editable && response.editable && !responseDescriptionKeys.has(response.key) && response.description === responseName(response)" type="button" size="xs" variant="ghost" @click="responseDescriptionKeys.add(response.key)">
                <Plus class="size-3.5" aria-hidden="true" />{{ t("specification.openapiDesignResponseAddDescription") }}
              </Button>
              <label v-if="responseDescriptionKeys.has(response.key) || response.description !== responseName(response)" class="openapi-design-field openapi-response-description">
                <span>{{ t("specification.openapiDescription") }}</span>
                <textarea :value="response.description" class="field-control" :name="`openapi-response-description-${response.key}`" :disabled="!props.editable || !response.editable" rows="3" @input="updateResponseDescription(response, ($event.target as HTMLTextAreaElement).value)" />
              </label>
              <OpenApiDesignExamples
                v-if="supportsResponseExample(activeResponseMedia(response))"
                :media-type="activeResponseMedia(response)"
                :rows="examplesFor(response, activeResponseMedia(response))"
                :editable="canEditExample(response, activeResponseMedia(response))"
                :can-generate="hasExampleSchema(activeResponseMedia(response), response.status)"
                @add="openExample(activeResponseMedia(response), response)"
                @generate="openExample(activeResponseMedia(response), response, undefined, true)"
                @edit="openExample(activeResponseMedia(response), response, $event)"
                @remove="setExample(response, activeResponseMedia(response), $event, null)"
              />
            </div>
          </div>
        </div>
        <p v-else class="openapi-design-empty">{{ t("specification.openapiDesignNoResponses") }}</p>
      </section>
    </fieldset>
    <OpenApiDesignAddResponseDialog v-model:open="addResponseOpen" :used-statuses="responses.map((response) => response.status)" @add="addResponse" />
    <OpenApiResponseExampleDialog
      v-if="exampleDialog"
      :target="{ status: exampleResponse?.status ?? '', mediaType: exampleDialog.mediaType }"
      :readonly="exampleResponse ? !canEditExample(exampleResponse, exampleDialog.mediaType) : !canEditRequestExample(exampleDialog.mediaType)"
      :initial-body="exampleDialog.body"
      :editing-name="exampleDialog.editingName"
      :names="dialogNames"
      @close="exampleDialog = undefined"
      @apply="applyExample"
    />
  </section>
</template>
