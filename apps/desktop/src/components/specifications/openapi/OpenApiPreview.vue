<script setup lang="ts">
import { extractPathParameterNames, resolvePathTemplate, restorePathTemplate } from "@/lib/specifications/openapi/workbench/request/openapiPathTemplate";
import { executeAgentRequest, serializeAgentRequest, base64ToBytes, selectedRequestProxy } from "@/lib/specifications/openapi/workbench/request/openapiRequestAgent";
import { resolveEnvironmentVariableValues, useEnvironmentLocalValues } from "@/stores/environmentLocalValues";
import { useRequestAgentStore } from "@/stores/requestAgentStore";
import OpenApiRequestProxyControl from "./workbench/request/OpenApiRequestProxyControl.vue";
import OpenApiResponseCookies from "./workbench/response/OpenApiResponseCookies.vue";
import { buildAgentResponseTelemetry } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";
import AppSelect from "@/components/ui/AppSelect.vue";
import OpenApiDebugActions from "./workbench/OpenApiDebugActions.vue";
import OpenApiBulkEditDialog from "./workbench/request/OpenApiBulkEditDialog.vue";
import { expandOpenApiParameterValue } from "@/lib/specifications/openapi/workbench/request/openapiParameterValues";
import { createOpenApiVariableResolver, OpenApiVariableError } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicVariables";
import { debugFieldSchema, debugFieldType, reconcileFormBulkRows } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";
import { RESPONSE_VISUALIZATION_DEMO } from "@/lib/specifications/openapi/workbench/response/openapiResponseVisualization";
import OpenApiDebugBody from "./workbench/request/OpenApiDebugBody.vue";
import OpenApiDebugResponsePanel from "./workbench/response/OpenApiDebugResponsePanel.vue";
import OpenApiResponseHeaders from "./workbench/response/OpenApiResponseHeaders.vue";
import OpenApiResponseConsole from "./workbench/response/OpenApiResponseConsole.vue";
import OpenApiExecutedRequest from "./workbench/response/OpenApiExecutedRequest.vue";
import { buildResponseTelemetry, observeResponseResourceTiming, requestBodyByteSize, type OpenApiResponseTelemetry } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";
import { describeExecutedRequest } from "@/lib/specifications/openapi/workbench/response/openapiExecutedRequest";
import type { DebugParameter } from "./workbench/request/OpenApiDebugParameterTable.vue";
import OpenApiDebugParameterSection from "./workbench/request/OpenApiDebugParameterSection.vue";
import OpenApiDebugSplitPane from "./workbench/OpenApiDebugSplitPane.vue";
import OpenApiDebugSettings from "./workbench/request/OpenApiDebugSettings.vue";
import OpenApiDebugFormTable, { type DebugFormRow } from "./workbench/request/OpenApiDebugFormTable.vue";
const authTemplateExample = "{{TOKEN}}";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { getActivePinia } from "pinia";
import { Braces, ExternalLink, FileJson, LoaderCircle, RotateCcw, Trash2, Upload, X } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
import { formatDate } from "@/lib/core";
import {
  expandOpenApiServerUrl,
  formatOpenApiExample,
  generateOpenApiExample,
  getOpenApiMediaExamples,
  getOpenApiOperations,
  getOpenApiServers,
  isRecord,
  openApiMethodClass,
  mergeOpenApiSchema,
  parseOpenApiContent,
  resolveOpenApiRef,
  type OpenApiObject,
} from "@/lib/specifications/openapi/contract/openapi";
import { validateOpenApiSchemaInWorker } from "@/lib/specifications/openapi/contract/openapiSchemaValidationClient";
import type { OpenApiSchemaValidationResult } from "@/lib/specifications/openapi/contract/openapiSchemaValidation";
import { getOperationParameters, readOpenApiPathParameter, readOpenApiQueryParameter, serializeOpenApiParameter } from "@/lib/specifications/openapi/contract/openapiParameters";
import { useWorkspaceStore } from "@/stores/workspace";
import { useEnvironmentContextStore } from "@/stores/environmentContextStore";
import { prefixOpenApiRequestPath } from "@/lib/specifications/openapi/environments/openapiEnvironments";
import { publicEnvironmentVariables, readPublicEnvironments } from "@/lib/specifications/openapi/environments/openapiPublicEnvironments";
import type { AgentResponse, OpenApiEnvironment, OpenApiEnvironmentVariable, PublicOpenApiEnvironment } from "@/types/api";
import { appendWorkbenchHistory, clearWorkbenchHistory, loadWorkbenchHistory, removeWorkbenchHistory, type WorkbenchHistoryEntry, type WorkbenchRequest, type OpenApiParameterConfiguration, type WorkbenchRow, type WorkbenchResponse } from "@/lib/specifications/openapi/workbench/openapiWorkbench";
import { parseCurlImport } from "@/lib/specifications/openapi/workbench/request/openapiCurl";
import { createOpenApiScriptHttpHost } from "@/lib/specifications/openapi/workbench/scripts/openapiScriptHttp";
import { OpenApiScriptError, runOpenApiScriptSequence, scriptVariables, type OpenApiScriptTest, type OpenApiScriptInput, type OpenApiScriptOutput } from "@/lib/specifications/openapi/workbench/scripts/openapiScriptSandbox";
import { buildOAuthAuthorizationUrl, createOAuthCodeChallenge, createOAuthCodeVerifier, createOAuthState, formEncode, isOAuthTokenExpired, isSafeOAuthEndpoint, parseOAuthTokenResponse, type OAuthFlow, type OAuthToken } from "@/lib/specifications/openapi/workbench/request/openapiOAuth";
import { applyOpenApiAuth, defaultAuthConfig, emptyAuthInput, redactAuthRequest, sanitizeAuthConfig, type OpenApiAuthConfig, type OpenApiAuthInput } from "@/lib/specifications/openapi/workbench/request/openapiAuth";
import { responseCaptureTarget, type CapturedResponseExample, type ResponseExampleInput } from "@/lib/specifications/openapi/design/openapiResponseExamples";
import OpenApiResponseViewer from "./workbench/response/OpenApiResponseViewer.vue";
import OpenApiResponseExampleDialog from "./design/OpenApiResponseExampleDialog.vue";
import OpenApiAuthRequirements from "./workbench/request/OpenApiAuthRequirements.vue";
import OpenApiOperationDocument from "./documentation/OpenApiOperationDocument.vue";
import OpenApiPayloadEditor from "./shared/OpenApiPayloadEditor.vue";
import OpenApiScriptSequence from "./workbench/scripts/OpenApiScriptSequence.vue";
import OpenApiDesignPanel from "./design/OpenApiDesignPanel.vue";
import type { OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import OpenApiRequestTarget from "./workbench/request/OpenApiRequestTarget.vue";
import OpenApiRequestEnvironment, { type RequestEnvironmentChoice } from "./workbench/request/OpenApiRequestEnvironment.vue";
import OpenApiOperationTabs, { type OpenApiOperationTab } from "./OpenApiOperationTabs.vue";

const props = withDefaults(
  defineProps<{
    content: string;
    focusOperationKey?: string;
    showOperationId?: boolean;
    schemaDisplay?: "summary" | "full";
    serviceId?: string;
    environments?: OpenApiEnvironment[];
    publicEnvironments?: PublicOpenApiEnvironment[];
    defaultEnvironmentId?: string | null;
    defaultServerUrl?: string;
    specificationTitle?: string;
    specificationVersion?: number;
    designDraftScope?: string;
    editable?: boolean;
    reader?: boolean;
    embeddedWorkbench?: boolean;
    publicTryIt?: boolean;
    navigateModel?: (reference: string) => void;
    saveOperation?: (content: string, change?: OpenApiOperationChange) => Promise<boolean>;
  }>(),
  {
    showOperationId: true,
    schemaDisplay: "summary",
    environments: () => [],
    publicEnvironments: () => [],
    defaultEnvironmentId: null,
    defaultServerUrl: "",
    specificationTitle: "",
    specificationVersion: 0,
    editable: false,
    publicTryIt: false,
  },
);
const t = translate;
const workspace = props.serviceId ? useWorkspaceStore() : null;
const environmentContext = getActivePinia() ? useEnvironmentContextStore() : null;
const requestAgent = getActivePinia() ? useRequestAgentStore() : null;
const localEnvironmentValues = getActivePinia() ? useEnvironmentLocalValues() : null;
const environmentSourceId = `openapi:${props.serviceId ?? "public"}`;
const requestValues = ref<Record<string, string>>({});
const requestEnabled = ref<Record<string, boolean>>({});
const customQueries = ref<WorkbenchRow[]>([]);
const customHeaders = ref<WorkbenchRow[]>([]);
const cookies = ref<WorkbenchRow[]>([]);
type BulkEditSection = "query" | "header" | "cookie";
const requestBodyText = ref("");
const requestExampleName = ref("");
const requestFile = ref<File | null>(null);
const importedBinaryBody = ref(false);
const requestMediaType = ref("");
const requestMethodOverride = ref("");
const requestUrlOverride = ref("");
const standaloneRequestMode = ref(false);
const replaceExplicitQuery = ref(false);
type FormRow = DebugFormRow;
const formRows = ref<FormRow[]>([]);
const portableEnvironments = computed(() => (props.publicEnvironments.length ? props.publicEnvironments : readPublicEnvironments(parseOpenApiContent(props.content))));
const localEnvironmentId = ref<string | null>(props.defaultEnvironmentId ?? (portableEnvironments.value.length ? "public:0" : null));
const selectedEnvironmentId = computed<string | null>({
  get: () => (props.serviceId && !props.publicTryIt && environmentContext ? environmentContext.selectionFor(environmentSourceId) : localEnvironmentId.value),
  set: (value) => {
    if (props.serviceId && !props.publicTryIt && environmentContext) environmentContext.selectFor(environmentSourceId, value);
    else localEnvironmentId.value = value;
  },
});
const publicVariableOverrides = ref<Record<string, Record<string, string>>>({});
const serverSelections = ref<Record<string, string | null>>({});
const serverVariableValues = ref<Record<string, string>>({});
const selectedAuthScheme = ref("specification");
const authInputs = ref<Record<string, OpenApiAuthInput>>({});
const authAlternative = ref(0);
function authField(field: keyof OpenApiAuthInput) {
  return computed({
    get: () => (Object.hasOwn(authInputs.value, selectedAuthScheme.value) ? authInputs.value[selectedAuthScheme.value]![field] : ""),
    set: (value: string) => {
      authInputs.value = { ...authInputs.value, [selectedAuthScheme.value]: { ...emptyAuthInput(), ...authInputs.value[selectedAuthScheme.value], [field]: value } };
    },
  });
}
const authValue = authField("value");
const authUsername = authField("username");
const authPassword = authField("password");
const currentAuthConfig = computed<OpenApiAuthConfig>(() => ({
  mode: selectedAuthScheme.value === "none" ? "none" : selectedAuthScheme.value === "specification" ? "specification" : "scheme",
  alternative: authAlternative.value,
  ...(selectedAuthScheme.value !== "none" && selectedAuthScheme.value !== "specification" ? { scheme: selectedAuthScheme.value } : {}),
  inputs: structuredCloneAuthInputs(authInputs.value),
}));
function structuredCloneAuthInputs(inputs: Record<string, OpenApiAuthInput>) {
  return Object.fromEntries(Object.entries(inputs).map(([name, input]) => [name, { ...input }]));
}
let oauthContextRevision = 0;
let activeOAuthPopup: Window | null = null;
watch(
  [selectedEnvironmentId, () => props.focusOperationKey, () => props.content],
  () => {
    oauthContextRevision += 1;
    activeOAuthPopup?.close();
    oauthToken.value = null;
    oauthTokenScheme.value = "";
    oauthStatus.value = "";
    oauthError.value = "";
  },
  { flush: "sync" },
);
const oauthClientId = ref("");
const oauthScopes = ref("");
const oauthToken = ref<OAuthToken | null>(null);
const oauthTokenScheme = ref("");
const oauthBusy = ref(false);
const oauthError = ref("");
const oauthStatus = ref("");
const running = ref(false);
const cancelRequest = ref<AbortController | null>(null);
const requestError = ref("");
const requestErrorPhase = ref<"before" | "after" | null>(null);
const requestTarget = ref<{ focusUrl: () => void }>();
const requestUrlError = ref("");
const showComputedHeaders = ref(false);
const generatedHeaderEnabled = ref<Record<string, boolean>>({});
const formBulkOpen = ref(false);

const responsePreview = ref<{
  status: number;
  statusText: string;
  duration: number;
  body: string;
  truncated: boolean;
  headers: Array<[string, string]>;
  contentType: string;
  scriptLogs?: string[];
  scriptError?: string;
  scriptTests?: OpenApiScriptTest[];
  visualize?: string;
  imageDataUrl?: string;
  fileDataUrl?: string;
  fileName?: string;
  binarySize?: number;
  rawBytes?: Uint8Array;
  agent?: AgentResponse;
  receivedBytes: number;
  metrics: OpenApiResponseTelemetry;
} | null>(null);
type ExecutedRequest = {
  method: string;
  url: string;
  headers: Array<[string, string]>;
  body: string;
  mediaType: string;
  bodyBytes?: number;
  operationExampleEligible: boolean;
};
const executedRequest = ref<ExecutedRequest | null>(null);
const selectedResponseDefinition = ref("");
const responseTab = ref("body");
const responseViewTabs = computed<OpenApiOperationTab[]>(() => [
  { value: "body", id: "openapi-response-tab-body", label: t(!responsePreview.value && requestError.value ? "specification.openapiResponseDetails.failed" : "specification.openapiResponseDetails.body") },
  ...(responsePreview.value || !requestError.value ? [{ value: "cookies", id: "openapi-response-tab-cookies", label: t("specification.openapiResponseDetails.cookie") }] : []),
  { value: "headers", id: "openapi-response-tab-headers", label: t("specification.openapiResponseDetails.header"), count: responsePreview.value?.headers.length },
  { value: "console", id: "openapi-response-tab-console", label: t("specification.openapiResponseDetails.console"), count: scriptLogs.value.length + scriptTests.value.length || (scriptError.value ? 1 : undefined) },
  { value: "request", id: "openapi-response-tab-request", label: t("specification.openapiExecutedRequest") },
]);
const responseCopyStatus = ref("");
const activeOperationView = ref<"docs" | "debug" | "design">(props.embeddedWorkbench ? "debug" : "docs");
const operationTabs = computed(() => [
  { value: "docs", id: "openapi-operation-documentation-tab", label: t("specification.openapiDocumentationTab") },
  ...(props.serviceId && !props.publicTryIt ? [{ value: "design", id: "openapi-operation-design-tab", label: t("specification.openapiDesignTab") }] : []),
  ...(props.serviceId || props.publicTryIt ? [{ value: "debug", id: "openapi-operation-debug-tab", label: t("specification.openapiDebugTab") }] : []),
]);
const debugSectionTabs = computed(() => [
  { value: "params", id: "openapi-debug-section-tab-params", label: t("specification.openapiDebugParams"), count: pathParameters.value.length + queryParameters.value.length + customQueries.value.filter((row) => row.enabled && row.name).length },
  { value: "body", id: "openapi-debug-section-tab-body", label: t("specification.openapiDebugBody") },
  { value: "headers", id: "openapi-debug-section-tab-headers", label: t("specification.openapiDebugHeaders") },
  { value: "cookies", id: "openapi-debug-section-tab-cookies", label: t("specification.openapiDebugCookies") },
  { value: "auth", id: "openapi-debug-section-tab-auth", label: t("specification.openapiDebugAuth") },
  { value: "before", id: "openapi-debug-section-tab-before", label: t("specification.openapiDebugBefore"), count: preRequestScripts.value.filter((script) => script.trim()).length },
  { value: "after", id: "openapi-debug-section-tab-after", label: t("specification.openapiDebugAfter"), count: postResponseScripts.value.filter((script) => script.trim()).length },
  { value: "settings", id: "openapi-debug-section-tab-settings", label: t("specification.openapiDebugSettings") },
]);
const designViewVisited = ref(false);
const showResponseExample = ref(false);
const capturedResponseExample = ref<CapturedResponseExample | null>(null);
const extractExampleButton = ref<HTMLButtonElement>();
watch(activeOperationView, (view) => {
  if (view === "design") designViewVisited.value = true;
});
type DebugSection = "params" | "body" | "headers" | "cookies" | "auth" | "before" | "after" | "settings";
const activeDebugSection = ref<DebugSection>("params");
const historyEntries = ref<WorkbenchHistoryEntry[]>([]);
const showHistory = ref(false);
const showCurlImport = ref(false);
const curlImportText = ref("");
const curlImportError = ref("");
const curlImportPreview = computed(() => (curlImportText.value.trim() ? parseCurlImport(curlImportText.value) : null));
const curlPreviewError = computed(() => (curlImportPreview.value && "error" in curlImportPreview.value ? t(`specification.openapiCurlErrors.${curlImportPreview.value.error.code}`, { option: curlImportPreview.value.error.option ?? "" }) : ""));
const contractResult = ref<{ ok: boolean; errors: string[] } | null>(null);
const validatingResponse = ref(false);
let responseValidationAbort: AbortController | null = null;

const preRequestScripts = ref<string[]>([]);
const postResponseScripts = ref<string[]>([]);
const requestTimeoutSeconds = ref(20);
const followRedirects = ref(true);
const autoValidateResponse = ref(false);
const scriptLogs = ref<string[]>([]);
const scriptTests = ref<OpenApiScriptTest[]>([]);
let executionScriptLocals: Record<string, string> | undefined;
let executionScriptScopes: OpenApiScriptInput["scopes"];
const scriptError = ref("");
const scriptSnippets = computed(() => [
  { label: t("specification.openapiScriptSnippetVariable"), value: 'const value = specx.environment.get("key");\nspecx.variables.set("name", value ?? "");' },
  { label: t("specification.openapiScriptSnippetLog"), value: 'specx.console.log("environment", specx.environment.get("key"));' },
  { label: t("specification.openapiScriptSnippetHeader"), value: 'specx.request.setHeader("X-Example", "value");' },
  { label: t("specification.openapiScriptSnippetRequest"), value: "const response = await pm.sendRequest(pm.request.url.toString());\nconsole.log(response.code, response.text());" },
]);
const postResponseSnippets = computed(() => [
  ...scriptSnippets.value.filter((snippet) => snippet.label !== t("specification.openapiScriptSnippetHeader")),
  { label: t("specification.openapiScriptSnippetAssertion"), value: 'pm.test("HTTP 200", () => pm.response.to.have.status(200));\npm.test("JSON response", () => pm.expect(pm.response.json()).to.be.an("object"));' },
  { label: t("specification.openapiVisualizationDemo"), value: RESPONSE_VISUALIZATION_DEMO },
]);
function addVisualizationDemo() {
  if (!postResponseScripts.value.includes(RESPONSE_VISUALIZATION_DEMO)) postResponseScripts.value = [...postResponseScripts.value.filter((script) => script.trim()), RESPONSE_VISUALIZATION_DEMO];
  activeDebugSection.value = "after";
}
const document = computed(() => parseOpenApiContent(props.content));
const info = computed(() => (isRecord(document.value?.info) ? document.value.info : null));
const operations = computed(() => getOpenApiOperations(props.content));
const focusedEntry = computed(() => operations.value.find((entry) => entry.identityKey === props.focusOperationKey));
const servers = computed(() => getOpenApiServers(document.value, focusedEntry.value));
const parameters = computed(() => (focusedEntry.value ? getOperationParameters(document.value, focusedEntry.value) : []));
const requestBody = computed(() => (isRecord(focusedEntry.value?.operation.requestBody) ? resolveOpenApiRef(document.value, focusedEntry.value.operation.requestBody) : null));
const responses = computed(() => {
  if (!isRecord(focusedEntry.value?.operation.responses)) return [];
  return Object.entries(focusedEntry.value.operation.responses).map(([status, response]) => [status, resolveOpenApiRef(document.value, response) ?? response] as [string, unknown]);
});
const responseDefinitionOptions = computed(() => responses.value.map(([code, response]) => ({ code, label: isRecord(response) && typeof response.description === "string" ? response.description : code })));
const selectedResponseDefinitionValue = computed({
  get: () => selectedResponseDefinition.value || responseDefinitionOptions.value[0]?.code || "",
  set: (value: string) => {
    selectedResponseDefinition.value = value;
  },
});
const security = computed(() => {
  return securityRequirements(focusedEntry.value?.operation);
});
const securitySchemes = computed(() => {
  const components = record(document.value?.components);
  const schemes = record(components?.securitySchemes);
  return schemes
    ? Object.entries(schemes).flatMap(([name, scheme]) => {
        const resolved = resolveOpenApiRef(document.value, scheme);
        return resolved ? [[name, resolved] as [string, Record<string, unknown>]] : [];
      })
    : [];
});

const parameterMetadata = ref<Record<string, OpenApiParameterConfiguration & { name?: string; type?: string; description?: string }>>({});
const requestPathTemplate = computed(() => requestUrlOverride.value || focusedEntry.value?.path || "");
const requestUrlTemplate = computed(() => (showEnvironmentPrefix.value ? requestPathTemplate.value : requestUrlOverride.value || prefixOpenApiRequestPath(previewServerUrl.value, requestPathTemplate.value)));
const requestParameters = computed<OpenApiObject[]>(() => {
  const defined = parameters.value.filter((p) => typeof p.name === "string" && typeof p.in === "string");
  const paths = extractPathParameterNames(requestUrlTemplate.value).map((name) => defined.find((p) => p.in === "path" && p.name === name) ?? { name, in: "path", required: true, schema: { type: "string" } });
  return [...defined.filter((p) => p.in !== "path"), ...paths].map((p) => {
    const key = `${p.in}:${p.name}`,
      patch = parameterMetadata.value[key];
    return {
      ...p,
      ...(patch?.name ? { name: patch.name } : {}),
      ...(patch?.description !== undefined ? { description: patch.description } : {}),
      ...(patch?.type ? { schema: patch.type.startsWith("#/") ? { $ref: patch.type } : { type: patch.type } } : {}),
      ...(patch?.schema ? { schema: patch.schema } : {}),
      ...(patch?.style ? { style: patch.style } : {}),
      ...(patch?.explode !== undefined ? { explode: patch.explode } : {}),
      "x-specx-editor-key": key,
    };
  });
});
const pathParameters = computed(() => requestParameters.value.filter((parameter) => parameter.in === "path"));
const queryParameters = computed(() => requestParameters.value.filter((parameter) => parameter.in === "query"));
const headerParameters = computed(() => requestParameters.value.filter((parameter) => parameter.in === "header"));
const cookieParameters = computed(() => requestParameters.value.filter((parameter) => parameter.in === "cookie"));
const requestParameterSerializationErrors = computed(() =>
  [...requestParameters.value, ...customQueries.value.filter((row) => row.enabled).map(customQueryParameter)].filter((parameter) => {
    if (requestEnabled.value[parameterKey(parameter)] === false) return false;
    const value = typeof parameter["x-specx-custom-value"] === "string" ? parameter["x-specx-custom-value"] : parameterInputValue(parameter);
    if (value.includes("{{")) return false;
    return Boolean(value.trim()) && !serializeOperationParameter(parameter, expand(value, previewVariables.value)).ok;
  }),
);
const requestBodyMedia = computed(() => {
  if (!requestBody.value || !isRecord(requestBody.value.content)) return [];
  return Object.entries(requestBody.value.content).filter(([, media]) => isRecord(media));
});
const requestMediaTypeIsJson = computed(() => requestMediaType.value.toLowerCase().split(";", 1)[0]?.endsWith("json") ?? false);
const requestBodyExamples = computed(() => {
  const media = requestBodyMedia.value.find(([mediaType]) => mediaType === requestMediaType.value) ?? requestBodyMedia.value[0];
  return media ? openApiMediaExamples(media[1], media[0]) : [];
});
const binaryRequestBody = computed(() => {
  const media = requestBodyMedia.value.find(([mediaType]) => mediaType === requestMediaType.value);
  const schema = isRecord(media?.[1]) ? mergeOpenApiSchema(document.value, media[1].schema) : null;
  return Boolean(importedBinaryBody.value || (schema && (schema.format === "binary" || typeof schema.contentMediaType === "string")) || requestMediaType.value === "application/octet-stream");
});
const selectedEnvironment = computed(() => props.environments.find((environment) => environment.id === selectedEnvironmentId.value) ?? null);
const selectedEnvironmentServer = computed(() => {
  const environment = selectedEnvironment.value;
  return environment?.servers?.find((server) => server.id === serverSelections.value[environment.id]);
});
const environmentProxyMode = computed(() => selectedEnvironmentServer.value?.requestMode ?? selectedEnvironment.value?.requestMode ?? environmentContext?.activeGlobal?.requestMode);
const effectiveProxy = computed(() => (props.publicTryIt || !workspace ? "browser" : selectedRequestProxy(requestAgent?.mode ?? "auto", environmentProxyMode.value)));
const selectedPublicEnvironment = computed(() => portableEnvironments.value.find((_, index) => `public:${index}` === selectedEnvironmentId.value));
const availableEnvironments = computed(() => [...(props.publicTryIt ? [] : props.environments), ...portableEnvironments.value.map((item, index) => ({ id: `public:${index}`, name: item.name }))]);
const showEnvironmentPrefix = computed(() => !props.publicTryIt && props.environments.length > 0);
const environmentChoices = computed<RequestEnvironmentChoice[]>(() =>
  props.environments.flatMap((environment) => [
    { environmentId: environment.id, serverId: null, label: environment.name, url: environment.baseUrl },
    ...(environment.servers ?? []).map((server) => ({ environmentId: environment.id, serverId: server.id, label: t("environmentContext.serverChoice", { environment: environment.name, server: server.name }), url: server.baseUrl })),
  ]),
);
function selectRequestEnvironment(choice: RequestEnvironmentChoice) {
  serverSelections.value[choice.environmentId] = choice.serverId;
  selectedEnvironmentId.value = choice.environmentId;
}
const requestEnvironmentVariables = computed<OpenApiEnvironmentVariable[]>(() => [
  ...((props.publicTryIt ? [] : environmentContext?.activeGlobal?.variables.filter((variable) => variable.key.trim()).map((variable) => ({ key: variable.key, value: variable.value, sensitive: variable.sensitive, enabled: variable.enabled, hasValue: Boolean(variable.value) }))) ?? []),
  ...(selectedEnvironment.value?.variables ?? []),
]);
const globalLocalValues = computed(() => (!props.publicTryIt && environmentContext?.activeGlobal ? (localEnvironmentValues?.values(`${workspace?.catalog?.workspace.id}:global:${environmentContext.activeGlobal.id}`) ?? {}) : {}));
const serviceLocalValues = computed(() => (selectedEnvironment.value ? (localEnvironmentValues?.values(`${workspace?.catalog?.workspace.id}:${props.serviceId}:${selectedEnvironment.value.id}`) ?? {}) : {}));
function scriptCreatedVariables(scope: string, definitions: Array<{ key: string }>) {
  const known = new Set(definitions.map((row) => row.key));
  return Object.fromEntries(Object.entries(localEnvironmentValues?.scriptValues(scope) ?? {}).filter(([key]) => !known.has(key)));
}
const globalRequestVariables = computed(() => ({
  ...resolveEnvironmentVariableValues(props.publicTryIt ? [] : (environmentContext?.activeGlobal?.variables ?? []), globalLocalValues.value),
  ...(!props.publicTryIt && environmentContext?.activeGlobal ? scriptCreatedVariables(`${workspace?.catalog?.workspace.id}:global:${environmentContext.activeGlobal.id}`, environmentContext.activeGlobal.variables) : {}),
}));
const serviceRequestVariables = computed(() => ({
  ...resolveEnvironmentVariableValues(selectedEnvironment.value?.variables ?? [], serviceLocalValues.value),
  ...(selectedEnvironment.value ? scriptCreatedVariables(`${workspace?.catalog?.workspace.id}:${props.serviceId}:${selectedEnvironment.value.id}`, selectedEnvironment.value.variables) : {}),
}));
const requestExecutionVariables = computed(() => ({
  ...serverVariableValues.value,
  ...globalRequestVariables.value,
  ...serviceRequestVariables.value,
  ...(selectedPublicEnvironment.value ? { ...publicEnvironmentVariables(selectedPublicEnvironment.value), ...publicVariableOverrides.value[selectedEnvironmentId.value ?? ""] } : {}),
}));
const previewVariables = computed(() => ({
  ...serverVariableValues.value,
  ...resolveEnvironmentVariableValues(props.publicTryIt ? [] : (environmentContext?.activeGlobal?.variables ?? []), globalLocalValues.value, false),
  ...resolveEnvironmentVariableValues(selectedEnvironment.value?.variables ?? [], serviceLocalValues.value, false),
  ...(selectedPublicEnvironment.value ? { ...publicEnvironmentVariables(selectedPublicEnvironment.value), ...publicVariableOverrides.value[selectedEnvironmentId.value ?? ""] } : {}),
}));
const resolvedServerUrl = computed(() => (showEnvironmentPrefix.value ? (selectedEnvironmentServer.value?.baseUrl ?? selectedEnvironment.value?.baseUrl ?? "") : selectedPublicEnvironment.value?.baseUrl || props.defaultServerUrl || servers.value[0]?.url || ""));
const previewServerUrl = computed(() => expand(expandOpenApiServerUrl(document.value, resolvedServerUrl.value, previewVariables.value, focusedEntry.value), previewVariables.value));
const serverVariables = computed(() => {
  const server = servers.value.find((item) => item.url === resolvedServerUrl.value);
  const variables = server && isRecord(server.variables) ? server.variables : null;
  return variables ? (Object.entries(variables).filter(([, variable]) => isRecord(variable)) as Array<[string, Record<string, unknown>]>) : [];
});
const unsafeServerUrl = computed(() => Boolean(currentRequestUrl.value) && !isSafeRequestUrl(expand(currentRequestUrl.value, previewVariables.value)));
const authSchemes = computed(() => securitySchemes.value.map(([name, scheme]) => ({ name, scheme })));
const computedHeaderRows = computed(() => {
  const agent = effectiveProxy.value === "agent";
  const transportHelp = t(agent ? "specification.openapiAgentHeader" : "specification.openapiBrowserHeader");
  const explicit = new Set(currentRequest.value.headers.filter((row) => row.enabled && row.name.trim() && row.value.trim()).map((row) => row.name.toLowerCase()));
  const rows: Array<{ name: string; value: string; description: string }> = [];
  if (!explicit.has("accept")) rows.push({ name: "Accept", value: "*/*", description: transportHelp });
  if (agent) {
    rows.push({ name: "User-Agent", value: "SpecX-Request-Agent/1.0", description: transportHelp });
    rows.push({ name: "Accept-Encoding", value: t("specification.openapiCalculatedAtSend"), description: transportHelp });
  }
  try {
    rows.push({ name: "Host", value: new URL(currentRequestUrl.value, window.location.origin).host, description: transportHelp });
  } catch {
    /* Incomplete URL stays editable. */
  }
  const hasBody = Boolean(requestMediaType.value && (requestBodyText.value.trim() || formRows.value.some((row) => row.enabled && row.name.trim()) || requestFile.value));
  if (requestMediaType.value && hasBody) {
    rows.push({ name: "Content-Type", value: requestMediaType.value === "multipart/form-data" ? "multipart/form-data; boundary=…" : requestMediaType.value, description: t("specification.openapiComputedHeaderBody") });
    rows.push({ name: "Content-Length", value: t("specification.openapiCalculatedAtSend"), description: transportHelp });
  }
  const result = applyOpenApiAuth({ config: currentAuthConfig.value, schemes: authSchemes.value, requirements: standaloneRequestMode.value ? [] : security.value, url: currentRequestUrl.value, headers: new Headers(), variables: previewVariables.value, preview: true });
  for (const [name, value] of result.headers.entries()) rows.push({ name, value, description: t("specification.openapiComputedHeaderAuth") });
  if (!agent) rows.push({ name: "Cookie", value: t("specification.openapiBrowserCookieJar"), description: transportHelp });
  else {
    const cookie = currentRequest.value.cookies
      .filter((row) => row.enabled && row.name.trim())
      .map((row) => `${row.name}=${row.value}`)
      .join("; ");
    if (cookie) rows.push({ name: "Cookie", value: cookie, description: transportHelp });
  }
  return rows
    .filter((row, index, all) => !explicit.has(row.name.toLowerCase()) && all.findIndex((candidate) => candidate.name.toLowerCase() === row.name.toLowerCase()) === index)
    .map((row): DebugParameter => {
      const name = row.name.toLowerCase();
      const managed = !agent && ["host", "content-length", "user-agent", "accept", "accept-encoding"].includes(name);
      return {
        ...row,
        key: `generated:${name}`,
        type: "string",
        schema: { type: "string" },
        placeholder: "",
        required: false,
        enabled: managed || generatedHeaderEnabled.value[name] !== false,
        generated: true,
        managed,
        description: managed ? t("specification.openapiBrowserManagedHeader") : row.description,
      };
    });
});
const selectedOAuthScheme = computed(() => {
  if (selectedAuthScheme.value === "specification" || selectedAuthScheme.value === "none") return null;
  const item = authSchemes.value.find((candidate) => candidate.name === selectedAuthScheme.value);
  if (!item || (item.scheme.type !== "oauth2" && item.scheme.type !== "openIdConnect")) return null;
  return item;
});
const selectedOAuthToken = computed(() => (selectedOAuthScheme.value && oauthTokenScheme.value === selectedOAuthScheme.value.name ? oauthToken.value : null));
const selectedOAuthTokenExpired = computed(() => Boolean(selectedOAuthToken.value && isOAuthTokenExpired(selectedOAuthToken.value)));
const ignoresEnvironmentPrefix = computed(() => isAbsoluteHttpUrl(expand(requestPathTemplate.value, previewVariables.value)));
const currentRequestUrl = computed(() => {
  const target = mergeRequestQuery(mergeRequestPath(requestUrlTemplate.value, previewVariables.value), previewVariables.value);
  return showEnvironmentPrefix.value && !ignoresEnvironmentPrefix.value ? prefixOpenApiRequestPath(previewServerUrl.value, target) : target;
});
const requestUrlEditor = computed({
  get: () => mergeRequestQuery(requestUrlTemplate.value, previewVariables.value),
  set: (value: string) => {
    setRequestUrl(value);
  },
});
const currentRequest = computed<WorkbenchRequest>(() => ({
  method: requestMethodOverride.value || focusedEntry.value?.method.toUpperCase() || "GET",
  url: currentRequestUrl.value,
  headers: [
    ...(standaloneRequestMode.value ? [] : headerParameters.value.map((parameter) => ({ name: String(parameter.name), value: requestValues.value[parameterKey(parameter)] ?? placeholderFor(parameter), enabled: requestEnabled.value[parameterKey(parameter)] !== false }))),
    ...customHeaders.value,
  ],
  cookies: [...(standaloneRequestMode.value ? [] : cookieParameters.value.map((parameter) => ({ name: String(parameter.name), value: parameterInputValue(parameter), enabled: requestEnabled.value[parameterKey(parameter)] !== false }))), ...cookies.value],
  body: serializedRequestBody(),
  mediaType: requestMediaType.value,
  auth: sanitizeAuthConfig(currentAuthConfig.value),
  ...(isFormMediaType(requestMediaType.value) ? { form: formRows.value.map((row) => ({ name: row.name, value: row.value, enabled: row.enabled, type: row.isFile ? "file" : row.type })) } : {}),
}));
const requestMethodEditor = computed(() => currentRequest.value.method);
const requestBodyJsonValid = computed(() => {
  if (binaryRequestBody.value || !requestMediaTypeIsJson.value || !requestBodyText.value.trim()) return true;
  try {
    if (/\{\{\s*\$/.test(requestBodyText.value)) return true;
    JSON.parse(requestBodyText.value);
    return true;
  } catch {
    return false;
  }
});

type RequestDraftSnapshot = {
  parameterMetadata: Record<string, OpenApiParameterConfiguration & { name?: string; type?: string; description?: string }>;
  requestValues: Record<string, string>;
  requestEnabled: Record<string, boolean>;
  generatedHeaderEnabled: Record<string, boolean>;
  customQueries: WorkbenchRow[];
  customHeaders: WorkbenchRow[];
  cookies: WorkbenchRow[];
  requestBodyText: string;
  requestFile: File | null;
  importedBinaryBody: boolean;
  requestMediaType: string;
  formRows: FormRow[];
  requestMethodOverride: string;
  requestUrlOverride: string;
  standaloneRequestMode: boolean;
  replaceExplicitQuery: boolean;
  serverVariableValues: Record<string, string>;
  selectedAuthScheme: string;
  authInputs: Record<string, OpenApiAuthInput>;
  authAlternative: number;
  oauthClientId: string;
  oauthScopes: string;
  preRequestScripts: string[];
  postResponseScripts: string[];
};
const REQUEST_DRAFT_HISTORY_LIMIT = 100;
const requestDraftUndoStack: RequestDraftSnapshot[] = [];
const requestDraftRedoStack: RequestDraftSnapshot[] = [];
const requestDraftHistoryRevision = ref(0);
const requestFileIds = new WeakMap<File, number>();
let nextRequestFileId = 1;
let requestDraftBaseline: RequestDraftSnapshot | null = null;
let latestRequestDraft: RequestDraftSnapshot | null = null;
let requestDraftTimer: number | undefined;
let restoringRequestEditor = false;
const requestDraftDirty = computed(() => {
  return requestDraftHistoryRevision.value > 0 && requestDraftBaseline !== null && !sameRequestDraft(captureRequestDraft(), requestDraftBaseline);
});
const canUndoRequestDraft = computed(() => {
  return requestDraftHistoryRevision.value > 0 && (requestDraftUndoStack.length > 0 || requestDraftDirty.value);
});
const canRedoRequestDraft = computed(() => {
  return requestDraftHistoryRevision.value > 0 && requestDraftRedoStack.length > 0;
});

function captureRequestDraft(): RequestDraftSnapshot {
  return {
    parameterMetadata: structuredClone(JSON.parse(JSON.stringify(parameterMetadata.value))),
    requestValues: { ...requestValues.value },
    requestEnabled: { ...requestEnabled.value },
    generatedHeaderEnabled: { ...generatedHeaderEnabled.value },
    customQueries: customQueries.value.map((row) => ({ ...row })),
    customHeaders: customHeaders.value.map((row) => ({ ...row })),
    cookies: cookies.value.map((row) => ({ ...row })),
    requestBodyText: requestBodyText.value,
    requestFile: requestFile.value,
    importedBinaryBody: importedBinaryBody.value,
    requestMediaType: requestMediaType.value,
    formRows: formRows.value.map((row) => ({ ...row, files: row.files ? [...row.files] : undefined })),
    requestMethodOverride: requestMethodOverride.value,
    requestUrlOverride: requestUrlOverride.value,
    standaloneRequestMode: standaloneRequestMode.value,
    replaceExplicitQuery: replaceExplicitQuery.value,
    serverVariableValues: { ...serverVariableValues.value },
    selectedAuthScheme: selectedAuthScheme.value,
    authInputs: structuredCloneAuthInputs(authInputs.value),
    authAlternative: authAlternative.value,
    oauthClientId: oauthClientId.value,
    oauthScopes: oauthScopes.value,
    preRequestScripts: [...preRequestScripts.value],
    postResponseScripts: [...postResponseScripts.value],
  };
}

function cloneRequestDraft(snapshot: RequestDraftSnapshot): RequestDraftSnapshot {
  return {
    ...snapshot,
    parameterMetadata: structuredClone(snapshot.parameterMetadata),
    requestValues: { ...snapshot.requestValues },
    requestEnabled: { ...snapshot.requestEnabled },
    generatedHeaderEnabled: { ...snapshot.generatedHeaderEnabled },
    customQueries: snapshot.customQueries.map((row) => ({ ...row })),
    customHeaders: snapshot.customHeaders.map((row) => ({ ...row })),
    cookies: snapshot.cookies.map((row) => ({ ...row })),
    serverVariableValues: { ...snapshot.serverVariableValues },
    authInputs: structuredCloneAuthInputs(snapshot.authInputs),
    formRows: snapshot.formRows.map((row) => ({ ...row, files: row.files ? [...row.files] : undefined })),
    preRequestScripts: [...snapshot.preRequestScripts],
    postResponseScripts: [...snapshot.postResponseScripts],
  };
}

function requestDraftKey(snapshot: RequestDraftSnapshot): string {
  return JSON.stringify(snapshot, (_key, value: unknown) => {
    if (typeof File === "undefined" || !(value instanceof File)) return value;
    let id = requestFileIds.get(value);
    if (!id) {
      id = nextRequestFileId++;
      requestFileIds.set(value, id);
    }
    return { fileId: id };
  });
}

function sameRequestDraft(left: RequestDraftSnapshot, right: RequestDraftSnapshot): boolean {
  return requestDraftKey(left) === requestDraftKey(right);
}

function updateRequestDraftHistoryRevision() {
  requestDraftHistoryRevision.value += 1;
}

function clearRequestDraftTimer() {
  if (requestDraftTimer !== undefined) window.clearTimeout(requestDraftTimer);
  requestDraftTimer = undefined;
}

function commitPendingRequestDraft() {
  clearRequestDraftTimer();
  const current = captureRequestDraft();
  if (!latestRequestDraft || sameRequestDraft(current, latestRequestDraft)) return;
  requestDraftUndoStack.push(cloneRequestDraft(latestRequestDraft));
  if (requestDraftUndoStack.length > REQUEST_DRAFT_HISTORY_LIMIT) requestDraftUndoStack.shift();
  latestRequestDraft = cloneRequestDraft(current);
  requestDraftRedoStack.length = 0;
  updateRequestDraftHistoryRevision();
}

function queueRequestDraftChange() {
  const current = captureRequestDraft();
  if (latestRequestDraft && sameRequestDraft(current, latestRequestDraft)) {
    clearRequestDraftTimer();
    return;
  }
  clearRequestDraftTimer();
  requestDraftTimer = window.setTimeout(commitPendingRequestDraft, 250);
}

function resetRequestDraftHistory() {
  clearRequestDraftTimer();
  const current = captureRequestDraft();
  requestDraftUndoStack.length = 0;
  requestDraftRedoStack.length = 0;
  latestRequestDraft = cloneRequestDraft(current);
  requestDraftBaseline = cloneRequestDraft(current);
  updateRequestDraftHistoryRevision();
}

function restoreRequestDraft(snapshot: RequestDraftSnapshot) {
  const draft = cloneRequestDraft(snapshot);
  restoringRequestEditor = true;
  parameterMetadata.value = draft.parameterMetadata;
  requestValues.value = draft.requestValues;
  requestEnabled.value = draft.requestEnabled;
  generatedHeaderEnabled.value = draft.generatedHeaderEnabled;
  customQueries.value = draft.customQueries;
  customHeaders.value = draft.customHeaders;
  cookies.value = draft.cookies;
  requestBodyText.value = draft.requestBodyText;
  requestFile.value = draft.requestFile;
  importedBinaryBody.value = draft.importedBinaryBody;
  requestMediaType.value = draft.requestMediaType;
  formRows.value = draft.formRows;
  requestMethodOverride.value = draft.requestMethodOverride;
  requestUrlOverride.value = draft.requestUrlOverride;
  standaloneRequestMode.value = draft.standaloneRequestMode;
  replaceExplicitQuery.value = draft.replaceExplicitQuery;
  serverVariableValues.value = draft.serverVariableValues;
  selectedAuthScheme.value = draft.selectedAuthScheme;
  authInputs.value = draft.authInputs;
  authAlternative.value = draft.authAlternative;
  oauthClientId.value = draft.oauthClientId;
  oauthScopes.value = draft.oauthScopes;
  preRequestScripts.value = draft.preRequestScripts;
  postResponseScripts.value = draft.postResponseScripts;
  void nextTick(() => {
    restoringRequestEditor = false;
  });
}

function undoRequestDraft() {
  commitPendingRequestDraft();
  const target = requestDraftUndoStack.pop();
  if (!target) return;
  if (latestRequestDraft) requestDraftRedoStack.push(cloneRequestDraft(latestRequestDraft));
  restoreRequestDraft(target);
  latestRequestDraft = cloneRequestDraft(target);
  updateRequestDraftHistoryRevision();
}

function redoRequestDraft() {
  commitPendingRequestDraft();
  const target = requestDraftRedoStack.pop();
  if (!target) return;
  if (latestRequestDraft) requestDraftUndoStack.push(cloneRequestDraft(latestRequestDraft));
  restoreRequestDraft(target);
  latestRequestDraft = cloneRequestDraft(target);
  updateRequestDraftHistoryRevision();
}

function handleRequestDraftKeydown(event: KeyboardEvent) {
  if (activeOperationView.value !== "debug" || !(event.metaKey || event.ctrlKey) || event.altKey) return;
  const target = event.target;
  if (target instanceof HTMLElement && (target.isContentEditable || target.closest("input, textarea, select, [contenteditable='true'], .cm-editor"))) return;
  const key = event.key.toLowerCase();
  if (key === "z" && event.shiftKey) {
    if (!canRedoRequestDraft.value) return;
    event.preventDefault();
    redoRequestDraft();
  } else if (key === "z") {
    if (!canUndoRequestDraft.value) return;
    event.preventDefault();
    undoRequestDraft();
  } else if (key === "y" && canRedoRequestDraft.value) {
    event.preventDefault();
    redoRequestDraft();
  }
}

watch(selectedAuthScheme, (schemeName) => {
  const item = authSchemes.value.find((candidate) => candidate.name === schemeName);
  if (!item || (item.scheme.type !== "oauth2" && item.scheme.type !== "openIdConnect")) {
    oauthScopes.value = "";
    return;
  }
  const requiredScopes = securityRequirements(focusedEntry.value?.operation).find((requirement) => Array.isArray(requirement[schemeName]))?.[schemeName];
  const operationScopes = Array.isArray(requiredScopes) ? requiredScopes.filter((scope): scope is string => typeof scope === "string") : [];
  oauthScopes.value = [...new Set([...(item.scheme.type === "openIdConnect" ? ["openid"] : []), ...operationScopes])].join(" ");
});

watch(
  () => [props.focusOperationKey, props.content] as const,
  () => {
    cancelRequest.value?.abort();
    cancelRequest.value = null;
    running.value = false;
    requestValues.value = {};
    requestEnabled.value = Object.fromEntries(parameters.value.map((parameter) => [`${String(parameter.in)}:${String(parameter.name)}`, true]));
    generatedHeaderEnabled.value = {};
    customQueries.value = [];
    customHeaders.value = [];
    cookies.value = [];
    requestError.value = "";
    responsePreview.value = null;
    selectedResponseDefinition.value = responseDefinitionOptions.value[0]?.code ?? "";
    executedRequest.value = null;
    parameterMetadata.value = {};
    requestExampleName.value = "";
    responseTab.value = "body";
    responseCopyStatus.value = "";
    activeDebugSection.value = "params";
    requestMethodOverride.value = "";
    requestUrlOverride.value = "";
    // Public try-it still runs the selected operation. Only the workspace
    // persistence/environment features are isolated; operation parameters,
    // request examples and declared authentication remain available.
    standaloneRequestMode.value = false;
    replaceExplicitQuery.value = false;
    requestMediaType.value = requestBodyMedia.value[0]?.[0] ?? "";
    requestBodyText.value = requestBodyExample(requestMediaType.value);
    requestFile.value = null;
    importedBinaryBody.value = false;
    formRows.value = formRowsFromSchema(requestMediaType.value);
    selectedAuthScheme.value = "specification";
    serverVariableValues.value = Object.fromEntries(serverVariables.value.map(([key, variable]) => [key, typeof variable.default === "string" ? variable.default : Array.isArray(variable.enum) && typeof variable.enum[0] === "string" ? variable.enum[0] : ""]));
    authInputs.value = {};
    authAlternative.value = 0;
    historyEntries.value = props.serviceId ? loadWorkbenchHistory(props.serviceId).filter((entry) => entry.operationKey === props.focusOperationKey || entry.operationKey === "imported") : [];
    showHistory.value = false;
    showCurlImport.value = false;
    requestTimeoutSeconds.value = 20;
    followRedirects.value = true;
    autoValidateResponse.value = false;
    curlImportText.value = "";
    curlImportError.value = "";
    contractResult.value = null;
    setScriptSequences(undefined, undefined);
    scriptLogs.value = [];
    scriptTests.value = [];
    scriptError.value = "";
    void nextTick(resetRequestDraftHistory);
  },
  { immediate: true },
);
watch(() => captureRequestDraft(), queueRequestDraftChange);
watch([selectedEnvironmentId, selectedEnvironmentServer], () => {
  requestUrlError.value = "";
});
watch(
  preRequestScripts,
  () => {
    requestError.value = "";
    scriptError.value = "";
  },
  { deep: true },
);
watch(
  postResponseScripts,
  () => {
    if (responsePreview.value) responsePreview.value.scriptError = undefined;
    requestError.value = "";
    scriptError.value = "";
  },
  { deep: true },
);
watch(
  [requestValues, customHeaders, customQueries, cookies, requestBodyText, requestUrlOverride, selectedEnvironmentId, selectedAuthScheme],
  () => {
    if (!running.value) {
      requestError.value = "";
      requestErrorPhase.value = null;
    }
  },
  { deep: true },
);
function setScriptSequences(before: string | undefined, after: string | undefined) {
  preRequestScripts.value = before?.trim() ? [before] : [];
  postResponseScripts.value = after?.trim() ? [after] : [];
}
function openScriptHelp() {
  window.open("https://docs.apifox.com/postman-script-api", "_blank", "noopener,noreferrer");
}
watch(
  [() => props.environments, portableEnvironments, () => props.defaultEnvironmentId],
  ([, portable]) => {
    // The application toolbar owns service option registration. An editor can
    // mount before its provider loads; its temporary empty props must not erase
    // the persisted service selection.
    if (props.serviceId && !props.publicTryIt && environmentContext) return;
    if (!availableEnvironments.value.some((item) => item.id === localEnvironmentId.value)) localEnvironmentId.value = props.defaultEnvironmentId ?? (portable[0] ? "public:0" : null);
  },
  { immediate: true },
);
watch(portableEnvironments, () => {
  publicVariableOverrides.value = {};
  if (!props.serviceId && !localEnvironmentId.value && portableEnvironments.value.length) localEnvironmentId.value = "public:0";
});

watch(requestMediaType, (value) => {
  if (restoringRequestEditor) return;
  importedBinaryBody.value = false;
  requestFile.value = null;
  const example = requestBodyExamples.value[0];
  requestExampleName.value = example?.name ?? "";
  if (example && !isFormMediaType(value) && !binaryRequestBody.value) {
    requestBodyText.value = formatOpenApiExample(example.value, value);
  }
  if (!isFormMediaType(value)) return;
  const existingRows = formRows.value;
  const existingValues = new Map(existingRows.map((row): [string, string] => [row.name, row.value]));
  const schemaRows = formRowsFromSchema(value);
  const schemaNames = new Set(schemaRows.map((row) => row.name));
  formRows.value = [...schemaRows.map((row) => ({ ...row, value: existingValues.get(row.name) ?? row.value })), ...existingRows.filter((row) => row.name && !schemaNames.has(row.name))];
});
function placeholderFor(parameter: Record<string, unknown>): string {
  const schema = record(parameter.schema);
  const example = parameter.example ?? schema?.example ?? schema?.default;
  if (example !== undefined && example !== null) return isRecord(example) || Array.isArray(example) ? JSON.stringify(example) : String(example);
  if (schema?.type === "object" || isRecord(schema?.properties)) return "{}";
  if (schema?.type === "array") return "[]";
  if (schema?.type === "integer" || schema?.type === "number") return "1";
  if (schema?.type === "boolean") return "true";
  return "";
}

function parameterKey(parameter: Record<string, unknown>): string {
  if (typeof parameter["x-specx-editor-key"] === "string") return parameter["x-specx-editor-key"];
  return `${String(parameter.in ?? "")}:${String(parameter.name ?? "")}`;
}

function parameterInputValue(parameter: Record<string, unknown>): string {
  const value = requestValues.value[parameterKey(parameter)] ?? placeholderFor(parameter);
  return typeof value === "string" ? value : String(value);
}

function serializeOperationParameter(parameter: Record<string, unknown>, value: string, variables: Record<string, string> = previewVariables.value) {
  const schemaSource = record(parameter.schema);
  const schema = schemaSource ? (mergeOpenApiSchema(document.value, schemaSource) ?? schemaSource) : null;
  const resolvedValue = executionVariableResolver && variables === activeExecutionVariables ? expandOpenApiParameterValue(value, schema ?? {}, document.value, executionVariableResolver, variables) : expand(value, variables);
  return serializeOpenApiParameter(parameter, resolvedValue, schema);
}

function parameterSchema(parameter: Record<string, unknown>): OpenApiObject {
  const schemaSource = record(parameter.schema);
  return schemaSource ? ((mergeOpenApiSchema(document.value, schemaSource) ?? schemaSource) as OpenApiObject) : {};
}

function parameterTypeLabel(parameter: Record<string, unknown>): string {
  return debugFieldType(record(parameter.schema) ?? parameterSchema(parameter));
}

function parameterDescription(parameter: Record<string, unknown>): string {
  return typeof parameter.description === "string" ? parameter.description : typeof parameterSchema(parameter).description === "string" ? String(parameterSchema(parameter).description) : "";
}

function queryNamesManagedByParameter(parameter: Record<string, unknown>, source?: URLSearchParams): { names: string[]; prefixes: string[] } {
  const name = typeof parameter.name === "string" ? parameter.name : "";
  const style = typeof parameter.style === "string" ? parameter.style : "form";
  const explode = typeof parameter.explode === "boolean" ? parameter.explode : style === "form";
  const schemaSource = record(parameter.schema);
  const schema = schemaSource ? (mergeOpenApiSchema(document.value, schemaSource) ?? schemaSource) : null;
  const isObject = schema?.type === "object" || isRecord(schema?.properties);
  const propertyNames = isObject && isRecord(schema?.properties) ? Object.keys(schema.properties) : [];
  const names = [name];
  if (parameter.style === "deepObject") return { names, prefixes: [`${name}[`] };
  if (style === "form" && explode && isObject) {
    names.push(...propertyNames);
    if (!propertyNames.length) {
      const declaredNames = new Set(queryParameters.value.map((item) => String(item.name)));
      if (source) names.push(...[...source.keys()].filter((key) => !declaredNames.has(key)));
      try {
        const parsed: unknown = JSON.parse(requestValues.value[parameterKey(parameter)] ?? "{}");
        if (isRecord(parsed)) names.push(...Object.keys(parsed));
      } catch {
        // Invalid editor input is reported by parameter validation before sending.
      }
    }
  }
  return { names: names.filter(Boolean), prefixes: [] };
}

function hydrateOperationParametersFromUrl(url: URL, includeOpenApiParameters: boolean) {
  const queryNames = queryParameters.value.map((parameter) => String(parameter.name));
  const managedQueryNames = new Set<string>();
  const managedQueryPrefixes: string[] = [];
  if (includeOpenApiParameters) {
    for (const parameter of queryParameters.value) {
      const managed = queryNamesManagedByParameter(parameter, url.searchParams);
      for (const name of managed.names) managedQueryNames.add(name);
      managedQueryPrefixes.push(...managed.prefixes);
      const schemaSource = record(parameter.schema);
      const schema = schemaSource ? (mergeOpenApiSchema(document.value, schemaSource) ?? schemaSource) : null;
      const value = readOpenApiQueryParameter(parameter, url.href, schema, queryNames);
      requestValues.value[parameterKey(parameter)] = value ?? "";
    }
    const operationPath = focusedEntry.value?.path;
    if (operationPath) {
      for (const parameter of pathParameters.value) {
        const schemaSource = record(parameter.schema);
        const schema = schemaSource ? (mergeOpenApiSchema(document.value, schemaSource) ?? schemaSource) : null;
        const value = readOpenApiPathParameter(operationPath, url.pathname, parameter, schema);
        if (value !== undefined && !value.includes("{")) requestValues.value[parameterKey(parameter)] = value;
      }
    }
  }
  customQueries.value = [...url.searchParams.entries()].filter(([name]) => !managedQueryNames.has(name) && !managedQueryPrefixes.some((prefix) => name.startsWith(prefix))).map(([name, value]) => ({ name, value, enabled: true }));
}

let activeExecutionVariables: Record<string, string> = {};
let executionVariableResolver: ReturnType<typeof createOpenApiVariableResolver> | undefined;
function expand(value: string, variables: Record<string, string>): string {
  if (executionVariableResolver && variables === activeExecutionVariables) return executionVariableResolver.expand(value, variables);
  return value.replace(/\{\{([^}]+)\}\}|<<([^>]+)>>/g, (match, moustache: string | undefined, angle: string | undefined) => {
    const key = (moustache ?? angle ?? "").trim();
    return variables[key] ?? match;
  });
}

function resolveStandaloneUrl(value: string, baseServer: string, variables: Record<string, string>): string {
  const decodedVariables = executionVariableResolver ? value.replace(/%7b%7b(?:(?!%7d%7d).)*%7d%7d/gi, (token) => encodeURIComponent(expand(decodeURIComponent(token), variables))) : value;
  const expanded = expand(decodedVariables, variables);
  try {
    return new URL(expanded, baseServer || window.location.origin).toString();
  } catch {
    return expanded;
  }
}

function isSafeRequestUrl(value: string): boolean {
  const withPlaceholders = value.replace(/\{\{[^}]+\}\}|<<[^>]+>>/g, "specx-placeholder");
  try {
    const parsed = new URL(withPlaceholders, window.location.origin);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function isAbsoluteHttpUrl(value: string): boolean {
  const expanded = value.replace(/\{\{[^}]+\}\}|<<[^>]+>>/g, "specx-placeholder").trim();
  return /^https?:\/\//iu.test(expanded) && isSafeRequestUrl(expanded);
}

function safeExternalUrl(value: string): string | null {
  return isSafeRequestUrl(value) ? value : null;
}

function securityRequirements(operation?: Record<string, unknown>): Record<string, unknown>[] {
  const configured = operation?.security ?? document.value?.security;
  return Array.isArray(configured) ? configured.filter(isRecord) : [];
}

function oauthFlowForScheme(scheme: Record<string, unknown>): OAuthFlow | null {
  if (scheme.type !== "oauth2") return null;
  const flows = record(scheme.flows);
  if (!flows) return null;
  const flow = record(flows.authorizationCode);
  if (!flow) return null;
  return {
    authorizationUrl: typeof flow.authorizationUrl === "string" ? flow.authorizationUrl : undefined,
    tokenUrl: typeof flow.tokenUrl === "string" ? flow.tokenUrl : undefined,
  };
}

async function resolveOAuthFlow(scheme: Record<string, unknown>): Promise<OAuthFlow> {
  const direct = oauthFlowForScheme(scheme);
  if (direct?.authorizationUrl && direct.tokenUrl) return direct;
  if (scheme.type === "openIdConnect" && typeof scheme.openIdConnectUrl === "string") {
    if (!isSafeOAuthEndpoint(scheme.openIdConnectUrl)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
    let response: Response;
    try {
      response = await fetch(scheme.openIdConnectUrl, { headers: { accept: "application/json" }, credentials: "omit", cache: "no-store" });
    } catch {
      throw new Error(t("specification.openapiOAuthDiscoveryFailed"));
    }
    if (!response.ok) throw new Error(t("specification.openapiOAuthDiscoveryFailed"));
    let discovery: unknown;
    try {
      discovery = await response.json();
    } catch {
      throw new Error(t("specification.openapiOAuthDiscoveryFailed"));
    }
    if (!isRecord(discovery) || typeof discovery.authorization_endpoint !== "string" || typeof discovery.token_endpoint !== "string") throw new Error(t("specification.openapiOAuthDiscoveryInvalid"));
    if (!isSafeOAuthEndpoint(discovery.authorization_endpoint) || !isSafeOAuthEndpoint(discovery.token_endpoint)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
    return { authorizationUrl: discovery.authorization_endpoint, tokenUrl: discovery.token_endpoint };
  }
  if (!direct?.authorizationUrl || !direct.tokenUrl) throw new Error(t("specification.openapiOAuthFlowUnsupported"));
  if (!isSafeOAuthEndpoint(direct.authorizationUrl) || !isSafeOAuthEndpoint(direct.tokenUrl)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
  return direct;
}

function oauthRedirectUri(): string {
  return new URL("/oauth/callback", window.location.origin).toString();
}

async function exchangeOAuthCode(tokenUrl: string, code: string, verifier: string, clientId: string): Promise<OAuthToken> {
  if (!isSafeOAuthEndpoint(tokenUrl)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
  let response: Response;
  try {
    response = await fetch(tokenUrl, {
      method: "POST",
      headers: { accept: "application/json", "content-type": "application/x-www-form-urlencoded" },
      credentials: "omit",
      cache: "no-store",
      body: formEncode({ grant_type: "authorization_code", code, client_id: clientId, redirect_uri: oauthRedirectUri(), code_verifier: verifier }),
    });
  } catch {
    throw new Error(t("specification.openapiOAuthTokenFailed"));
  }
  let payload: unknown = null;
  try {
    payload = await response.json();
  } catch {
    throw new Error(t("specification.openapiOAuthTokenInvalid"));
  }
  if (!response.ok) throw new Error(t("specification.openapiOAuthTokenFailed"));
  try {
    return parseOAuthTokenResponse(payload);
  } catch {
    throw new Error(t("specification.openapiOAuthTokenInvalid"));
  }
}

async function waitForOAuthCallback(popup: Window): Promise<{ code: string | null; state: string | null; error: string | null }> {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const expectedPath = new URL(oauthRedirectUri()).pathname;
    const timer = window.setInterval(() => {
      if (popup.closed) {
        window.clearInterval(timer);
        reject(new Error(t("specification.openapiOAuthWindowClosed")));
        return;
      }
      if (Date.now() - started > 120_000) {
        window.clearInterval(timer);
        popup.close();
        reject(new Error(t("specification.openapiOAuthTimeout")));
        return;
      }
      try {
        const callback = new URL(popup.location.href);
        if (callback.origin !== window.location.origin || callback.pathname !== expectedPath || (!callback.searchParams.has("code") && !callback.searchParams.has("error"))) return;
        window.clearInterval(timer);
        popup.close();
        resolve({ code: callback.searchParams.get("code"), state: callback.searchParams.get("state"), error: callback.searchParams.get("error") });
      } catch {
        // Cross-origin popup locations are intentionally unreadable until the redirect returns.
      }
    }, 250);
  });
}

async function authorizeOAuth() {
  const item = selectedOAuthScheme.value;
  if (!item || oauthBusy.value) return;
  const clientId = oauthClientId.value.trim();
  if (!clientId) {
    oauthError.value = t("specification.openapiOAuthClientIdRequired");
    return;
  }
  const popup = window.open("about:blank", "_blank", "popup,width=520,height=720");
  if (!popup) {
    oauthError.value = t("specification.openapiOAuthPopupBlocked");
    return;
  }
  const contextRevision = oauthContextRevision;
  const scopeInput = oauthScopes.value;
  activeOAuthPopup = popup;
  oauthBusy.value = true;
  oauthError.value = "";
  oauthStatus.value = "";
  try {
    const flow = await resolveOAuthFlow(item.scheme);
    if (!flow.authorizationUrl || !flow.tokenUrl) throw new Error(t("specification.openapiOAuthFlowUnsupported"));
    if (!isSafeOAuthEndpoint(flow.authorizationUrl) || !isSafeOAuthEndpoint(flow.tokenUrl)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
    const state = createOAuthState();
    const verifier = createOAuthCodeVerifier();
    const challenge = await createOAuthCodeChallenge(verifier);
    if (contextRevision !== oauthContextRevision) {
      oauthError.value = t("specification.openapiAuth.environmentChanged");
      return;
    }
    const scopes = scopeInput
      .split(/[\s,]+/u)
      .map((scope) => scope.trim())
      .filter(Boolean);
    popup.opener = null;
    popup.location.href = buildOAuthAuthorizationUrl({ authorizationUrl: flow.authorizationUrl, clientId, redirectUri: oauthRedirectUri(), state, codeChallenge: challenge, scopes });
    const callback = await waitForOAuthCallback(popup);
    if (contextRevision !== oauthContextRevision) {
      oauthError.value = t("specification.openapiAuth.environmentChanged");
      return;
    }
    if (callback.state !== state) throw new Error(t("specification.openapiOAuthStateInvalid"));
    if (callback.error) throw new Error(t("specification.openapiOAuthAuthorizationFailed"));
    if (!callback.code) throw new Error(t("specification.openapiOAuthCallbackInvalid"));
    const token = await exchangeOAuthCode(flow.tokenUrl, callback.code, verifier, clientId);
    if (contextRevision !== oauthContextRevision) {
      oauthError.value = t("specification.openapiAuth.environmentChanged");
      return;
    }
    oauthToken.value = token;
    oauthTokenScheme.value = item.name;
    oauthStatus.value = t("specification.openapiOAuthAuthorized");
  } catch (cause) {
    if (contextRevision !== oauthContextRevision) {
      oauthError.value = t("specification.openapiAuth.environmentChanged");
      return;
    }
    if (!popup.closed) popup.close();
    const knownMessages = [
      t("specification.openapiOAuthDiscoveryFailed"),
      t("specification.openapiOAuthDiscoveryInvalid"),
      t("specification.openapiOAuthFlowUnsupported"),
      t("specification.openapiOAuthUnsafeEndpoint"),
      t("specification.openapiOAuthTokenInvalid"),
      t("specification.openapiOAuthTokenFailed"),
      t("specification.openapiOAuthCryptoUnavailable"),
      t("specification.openapiOAuthWindowClosed"),
      t("specification.openapiOAuthTimeout"),
      t("specification.openapiOAuthAuthorizationFailed"),
      t("specification.openapiOAuthCallbackInvalid"),
      t("specification.openapiOAuthStateInvalid"),
    ];
    oauthError.value = cause instanceof Error && cause.message === "oauth_crypto_unavailable" ? t("specification.openapiOAuthCryptoUnavailable") : cause instanceof Error && knownMessages.includes(cause.message) ? cause.message : t("specification.openapiOAuthTokenFailed");
  } finally {
    activeOAuthPopup = null;
    oauthBusy.value = false;
  }
}

async function refreshOAuthToken() {
  const item = selectedOAuthScheme.value;
  const token = selectedOAuthToken.value;
  if (!item || !token?.refreshToken || oauthBusy.value) return;
  const contextRevision = oauthContextRevision;
  oauthBusy.value = true;
  oauthError.value = "";
  try {
    const flow = await resolveOAuthFlow(item.scheme);
    const clientId = oauthClientId.value.trim();
    if (!flow.tokenUrl || !clientId) throw new Error(t("specification.openapiOAuthRefreshUnavailable"));
    if (!isSafeOAuthEndpoint(flow.tokenUrl)) throw new Error(t("specification.openapiOAuthUnsafeEndpoint"));
    let response: Response;
    try {
      response = await fetch(flow.tokenUrl, {
        method: "POST",
        headers: { accept: "application/json", "content-type": "application/x-www-form-urlencoded" },
        credentials: "omit",
        cache: "no-store",
        body: formEncode({ grant_type: "refresh_token", refresh_token: token.refreshToken, client_id: clientId }),
      });
    } catch {
      throw new Error(t("specification.openapiOAuthTokenFailed"));
    }
    let payload: unknown;
    try {
      payload = await response.json();
    } catch {
      throw new Error(t("specification.openapiOAuthTokenInvalid"));
    }
    if (!response.ok) throw new Error(t("specification.openapiOAuthTokenFailed"));
    const refreshed = parseOAuthTokenResponse(payload);
    if (contextRevision !== oauthContextRevision) {
      oauthError.value = t("specification.openapiAuth.environmentChanged");
      return;
    }
    oauthToken.value = { ...refreshed, refreshToken: refreshed.refreshToken ?? token.refreshToken };
    oauthStatus.value = t("specification.openapiOAuthRefreshed");
  } catch (cause) {
    oauthError.value =
      cause instanceof Error && [t("specification.openapiOAuthRefreshUnavailable"), t("specification.openapiOAuthTokenFailed"), t("specification.openapiOAuthTokenInvalid"), t("specification.openapiOAuthUnsafeEndpoint")].includes(cause.message)
        ? cause.message
        : t("specification.openapiOAuthTokenFailed");
  } finally {
    oauthBusy.value = false;
  }
}

function clearOAuthToken() {
  oauthToken.value = null;
  oauthTokenScheme.value = "";
  authValue.value = "";
  oauthError.value = "";
  oauthStatus.value = t("specification.openapiOAuthCleared");
}

function executionAuthTokens(): Record<string, string> {
  const token = oauthToken.value;
  return token && !isOAuthTokenExpired(token) && oauthTokenScheme.value ? { [oauthTokenScheme.value]: `${token.tokenType} ${token.accessToken}` } : {};
}
class RequestAuthenticationError extends Error {
  constructor(
    message: string,
    readonly scheme: string,
  ) {
    super(message);
  }
}
function applySecurity(headers: Headers, url: string, variables: Record<string, string>, config: OpenApiAuthConfig, operation?: Record<string, unknown>, tokens: Record<string, string> = {}, disabledHeaders: string[] = []) {
  const runtimeConfig = executionVariableResolver ? { ...config, inputs: Object.fromEntries(Object.entries(config.inputs).map(([name, input]) => [name, { value: expand(input.value, variables), username: expand(input.username, variables), password: expand(input.password, variables) }])) } : config;
  const result = applyOpenApiAuth({ config: runtimeConfig, schemes: authSchemes.value, requirements: operation ? securityRequirements(operation) : [], url, headers, variables, tokens, disabledHeaders });
  if (result.issues.length) {
    const issue = result.issues[0]!;
    const scheme = authSchemes.value.find((item) => item.name === issue.scheme)?.scheme;
    if (issue.code === "missing" && (scheme?.type === "oauth2" || scheme?.type === "openIdConnect")) throw new RequestAuthenticationError(t("specification.openapiOAuthTokenRequired"), issue.scheme);
    throw new RequestAuthenticationError(t(`specification.openapiAuth.${issue.code}`, { scheme: issue.scheme }), issue.scheme);
  }
  return result;
}

function responseFileName(response: Response): string {
  const disposition = response.headers.get("content-disposition") ?? "";
  const encoded = disposition.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  const quoted = disposition.match(/filename\s*=\s*"([^"]+)"/i)?.[1];
  const plain = disposition.match(/filename\s*=\s*([^;]+)/i)?.[1]?.trim();
  let candidate = encoded ?? quoted ?? plain ?? "";
  try {
    candidate = encoded ? decodeURIComponent(encoded) : candidate;
  } catch {
    candidate = quoted ?? plain ?? "";
  }
  candidate = candidate.split(/[\\/]/).at(-1) ?? "";
  return Array.from(candidate, (character) => (character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127 ? character : ""))
    .join("")
    .trim();
}

function bytesToDataUrl(bytes: Uint8Array, contentType: string): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return `data:${contentType || "application/octet-stream"};base64,${btoa(binary)}`;
}

async function readResponse(response: Response): Promise<{ body: string; truncated: boolean; imageDataUrl?: string; fileDataUrl?: string; fileName?: string; binarySize?: number; receivedBytes: number; rawBytes: Uint8Array }> {
  const maxBytes = 1024 * 1024;
  const contentType = response.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase() ?? "";
  const imageResponse = contentType.startsWith("image/");
  const textResponse = !imageResponse && (!contentType || contentType.startsWith("text/") || /application\/(?:[^;]+\+)?(?:json|xml|javascript|x-javascript|yaml|x-yaml|x-www-form-urlencoded)/.test(contentType));
  if (!response.body) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    const bounded = bytes.subarray(0, maxBytes);
    if (imageResponse)
      return { rawBytes: bounded, body: "", truncated: bytes.byteLength > maxBytes, ...(bytes.byteLength <= maxBytes ? { imageDataUrl: bytesToDataUrl(bounded, contentType) } : {}), receivedBytes: bounded.byteLength, binarySize: bytes.byteLength, fileName: responseFileName(response) };
    return textResponse
      ? { rawBytes: bounded, body: new TextDecoder().decode(bounded), receivedBytes: bounded.byteLength, truncated: bytes.byteLength > maxBytes }
      : { rawBytes: bounded, body: "", truncated: bytes.byteLength > maxBytes, receivedBytes: bounded.byteLength, fileDataUrl: bytesToDataUrl(bounded, contentType), fileName: responseFileName(response), binarySize: Number(response.headers.get("content-length")) || bytes.byteLength };
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  let truncated = false;
  while (true) {
    const part = await reader.read();
    if (part.done) break;
    if (total + part.value.byteLength > maxBytes) {
      const accepted = part.value.slice(0, maxBytes - total);
      chunks.push(accepted);
      total += accepted.byteLength;
      truncated = true;
      await reader.cancel();
      break;
    }
    chunks.push(part.value);
    total += part.value.byteLength;
  }
  const bytes = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  if (imageResponse) {
    return { rawBytes: bytes, body: "", truncated, receivedBytes: total, ...(truncated ? {} : { imageDataUrl: bytesToDataUrl(bytes, contentType) }), binarySize: Number(response.headers.get("content-length")) || total, fileName: responseFileName(response) };
  }
  if (!textResponse) {
    return {
      rawBytes: bytes,
      body: "",
      truncated,
      receivedBytes: total,
      fileDataUrl: bytesToDataUrl(bytes, contentType),
      fileName: responseFileName(response),
      binarySize: Number(response.headers.get("content-length")) || total,
    };
  }
  return { rawBytes: bytes, body: new TextDecoder().decode(bytes), truncated, receivedBytes: total };
}

function formatRequestBody() {
  if (!requestBodyText.value.trim() || !requestMediaTypeIsJson.value) return;
  try {
    requestBodyText.value = `${JSON.stringify(JSON.parse(requestBodyText.value), null, 2)}\n`;
  } catch {
    requestError.value = t("specification.openapiInvalidJsonBody");
  }
}

function describeRequestBody(body: BodyInit | undefined): string {
  if (body === undefined) return "";
  if (typeof body === "string") return body;
  if (body instanceof File) return `[${body.name} · ${formatByteSize(body.size)}]`;
  if (body instanceof URLSearchParams) return body.toString();
  if (body instanceof FormData) {
    return [...body.entries()].map(([name, value]) => `${name}=${value instanceof File ? `[${value.name}]` : String(value)}`).join("&");
  }
  return "[binary request body]";
}

function resetRequest() {
  parameterMetadata.value = {};
  requestValues.value = {};
  requestEnabled.value = Object.fromEntries(parameters.value.map((parameter) => [parameterKey(parameter), true]));
  generatedHeaderEnabled.value = {};
  customQueries.value = [];
  customHeaders.value = [];
  cookies.value = [];
  requestBodyText.value = requestBodyExample(requestMediaType.value);
  requestFile.value = null;
  importedBinaryBody.value = false;
  formRows.value = formRowsFromSchema(requestMediaType.value);
  requestError.value = "";
  requestMethodOverride.value = "";
  requestUrlOverride.value = "";
  standaloneRequestMode.value = false;
  replaceExplicitQuery.value = false;
}

function requestBodyExample(mediaType = requestMediaType.value): string {
  const media = requestBodyMedia.value.find(([candidate]) => candidate === mediaType) ?? requestBodyMedia.value.find(([candidate]) => candidate === "application/json") ?? requestBodyMedia.value[0];
  if (!media) return "";
  const example = openApiMediaExamples(media[1], media[0])[0];
  if (example) {
    requestExampleName.value = example.name;
    return formatOpenApiExample(example.value, media[0]);
  }
  return "";
}

function openApiMediaExamples(media: unknown, mediaType: string) {
  const explicit = getOpenApiMediaExamples(document.value, media);
  if (explicit.length) return explicit;
  const schema = isRecord(media) ? media.schema : undefined;
  if (schema === undefined) return [];
  return [{ name: "generated", label: t("specification.openapiGeneratedExample", { mediaType }), description: "", value: generateOpenApiExample(document.value, schema) }];
}

function applyRequestExample() {
  const example = requestBodyExamples.value.find((candidate) => candidate.name === requestExampleName.value) ?? requestBodyExamples.value[0];
  if (!example) return;
  requestBodyText.value = formatOpenApiExample(example.value, requestMediaType.value);
}

function formRowsFromSchema(mediaType = requestMediaType.value || requestBodyMedia.value[0]?.[0] || ""): FormRow[] {
  const media = requestBodyMedia.value.find(([candidate]) => candidate === mediaType && isFormMediaType(candidate));
  const schema = isRecord(media?.[1]) ? mergeOpenApiSchema(document.value, media[1].schema) : null;
  const properties = schema && isRecord(schema.properties) ? Object.entries(schema.properties) : [];
  const required = new Set(Array.isArray(schema?.required) ? schema.required.filter((item): item is string => typeof item === "string") : []);
  return properties.map(([name, value]) => {
    const property = mergeOpenApiSchema(document.value, value) ?? record(value) ?? {};
    const item = isRecord(property.items) ? mergeOpenApiSchema(document.value, property.items) : null;
    const isFile = media?.[0] === "multipart/form-data" && (property.format === "binary" || typeof property.contentMediaType === "string" || item?.format === "binary" || typeof item?.contentMediaType === "string");
    return {
      name,
      value: isFile ? "" : placeholderFor({ schema: property }),
      type: isFile ? "file" : debugFieldType(record(value) ?? property),
      schema: property,
      description: typeof property.description === "string" ? property.description : "",
      enabled: true,
      isFile,
      files: [],
      multiple: property.type === "array",
      required: required.has(name),
    };
  });
}

function isFormMediaType(value: string): boolean {
  return value === "multipart/form-data" || value === "application/x-www-form-urlencoded";
}

function serializedRequestBody(): string {
  if (!requestMediaType.value || binaryRequestBody.value) return "";
  if (!isFormMediaType(requestMediaType.value)) return requestBodyText.value;
  return formRows.value
    .filter((row) => row.enabled && row.name.trim() && !row.isFile)
    .map((row) => `${encodeURIComponent(row.name.trim())}=${encodeURIComponent(row.value)}`)
    .join("&");
}

function selectRequestFile(event: Event) {
  requestFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
}

function requestFromEntry(entry: WorkbenchRequest) {
  restoringRequestEditor = true;
  const parsed = new URL(entry.url, window.location.origin);
  const operation = focusedEntry.value;
  // History and imported targets edit the temporary request; the operation definition stays intact.
  const isStandalone = !operation;
  standaloneRequestMode.value = isStandalone;
  const auth = entry.auth ?? defaultAuthConfig(isStandalone ? "none" : "specification");
  selectedAuthScheme.value = auth.mode === "scheme" ? auth.scheme! : auth.mode;
  authAlternative.value = auth.alternative;
  authInputs.value = structuredCloneAuthInputs(auth.inputs);
  replaceExplicitQuery.value = false;
  requestMethodOverride.value = props.publicTryIt ? "" : entry.method.toUpperCase();
  requestUrlOverride.value = operation ? restorePathTemplate(entry.url, operation.path) : entry.url;
  for (const parameter of requestParameters.value) {
    const key = parameterKey(parameter);
    if (parameter.in === "query" || parameter.in === "path") requestValues.value[key] = "";
    if (parameter.in === "header") requestValues.value[key] = entry.headers.find((header) => header.name.toLowerCase() === String(parameter.name).toLowerCase())?.value ?? "";
    if (parameter.in === "cookie") requestValues.value[key] = entry.cookies.find((cookie) => cookie.name === String(parameter.name))?.value ?? "";
  }
  hydrateOperationParametersFromUrl(parsed, !isStandalone);
  customHeaders.value = isStandalone ? [...entry.headers] : entry.headers.filter((header) => !headerParameters.value.some((parameter) => String(parameter.name).toLowerCase() === header.name.toLowerCase()));
  cookies.value = isStandalone ? [...entry.cookies] : entry.cookies.filter((cookie) => !cookieParameters.value.some((parameter) => String(parameter.name) === cookie.name));
  requestBodyText.value = entry.body;
  requestFile.value = null;
  importedBinaryBody.value = false;
  requestMediaType.value = entry.mediaType;
  formRows.value = isFormMediaType(entry.mediaType) ? Array.from(new URLSearchParams(entry.body), ([name, value]) => ({ name, value, enabled: true, files: [] })) : [];
  if (isFormMediaType(entry.mediaType)) {
    const parsedRows = formRows.value;
    const parsedByName = new Map(parsedRows.map((row) => [row.name, row]));
    const schemaRows = isStandalone ? [] : formRowsFromSchema(entry.mediaType);
    const schemaNames = new Set(schemaRows.map((row) => row.name));
    formRows.value = [...schemaRows.map((row) => ({ ...row, value: parsedByName.get(row.name)?.value ?? "" })), ...parsedRows.filter((row) => row.name && !schemaNames.has(row.name))];
  }
  for (const row of formRows.value) row.files = [];
  void nextTick(() => {
    restoringRequestEditor = false;
  });
}

function applyBulkRows(section: BulkEditSection, rows: WorkbenchRow[]) {
  const definitions = section === "query" ? queryParameters.value : section === "header" ? headerParameters.value : cookieParameters.value;
  const normalize = (name: string) => (section === "header" ? name.toLowerCase() : name);
  const matchedIndexes = new Set<number>();
  if (!standaloneRequestMode.value) {
    for (const parameter of definitions) {
      const key = parameterKey(parameter);
      const name = normalize(String(parameter.name));
      const index = rows.findIndex((row, rowIndex) => !matchedIndexes.has(rowIndex) && normalize(row.name) === name);
      if (index < 0) {
        requestValues.value[key] = "";
        requestEnabled.value[key] = false;
        continue;
      }
      const row = rows[index]!;
      matchedIndexes.add(index);
      parameterMetadata.value[key] = { ...parameterMetadata.value[key], ...(row.type ? { type: row.type } : {}), ...(row.description !== undefined ? { description: row.description } : {}) };
      requestValues.value[key] = row.value;
      requestEnabled.value[key] = row.enabled;
    }
  }
  const extras = rows.filter((_row, index) => !matchedIndexes.has(index)).map((row) => ({ ...row }));
  if (section === "query") {
    customQueries.value = extras;
    replaceExplicitQuery.value = true;
  } else if (section === "header") customHeaders.value = extras;
  else cookies.value = extras;
}

function mergeRequestPath(value: string, variables: Record<string, string> = {}): string {
  if (extractPathParameterNames(value).length) {
    const serializedValues = Object.fromEntries(
      pathParameters.value
        .filter((p) => requestEnabled.value[parameterKey(p)] !== false)
        .flatMap((p) => {
          const result = serializeOperationParameter(p, parameterInputValue(p), variables);
          return result.ok ? [[String(p.name), result.value.pathValue]] : [];
        }),
    );
    return resolvePathTemplate(value, serializedValues, (encoded) => encoded);
  }

  const operationPath = focusedEntry.value?.path;
  if (!operationPath || !pathParameters.value.length) return value;
  let parsed: URL;
  try {
    parsed = new URL(value, window.location.origin);
  } catch {
    return value;
  }
  // The initial slash is a route separator, not a segment to match against a
  // server's base path (for example /v2 before /pets/{id}).
  const templateSegments = operationPath.slice(1).split("/");
  const pathSegments = parsed.pathname.split("/");
  const offset = pathSegments.length - templateSegments.length;
  if (offset < 0) return value;
  if (templateSegments.some((segment, index) => !segment.includes("{") && decodeURIComponentSafe(pathSegments[offset + index] ?? "") !== decodeURIComponentSafe(segment))) return value;
  for (const parameter of pathParameters.value) {
    if (requestEnabled.value[parameterKey(parameter)] === false) continue;
    const name = String(parameter.name);
    const templateIndex = templateSegments.findIndex((segment) => segment.includes(`{${name}}`));
    if (templateIndex < 0) continue;
    const actualIndex = offset + templateIndex;
    const actualSegment = pathSegments[actualIndex];
    const templateSegment = templateSegments[templateIndex];
    if (actualSegment === undefined || templateSegment === undefined) continue;
    const match = /^([^{}]*)\{[^{}]+\}([^{}]*)$/u.exec(templateSegment);
    if (!match) continue;
    const [, before, after] = match;
    const rawValue = parameterInputValue(parameter);
    if (!rawValue) continue;
    const serialized = serializeOperationParameter(parameter, parameterInputValue(parameter), variables);
    if (!serialized.ok) {
      if (executionVariableResolver && variables === activeExecutionVariables) throw new Error(t("specification.openapiParameterSerializationError"));
      continue;
    }
    const encodedValue = serialized.value.pathValue;
    const middle = actualSegment.startsWith(before ?? "") && actualSegment.endsWith(after ?? "") ? actualSegment.slice((before ?? "").length, actualSegment.length - (after ?? "").length || undefined) : "";
    if (middle || actualSegment === `${before ?? ""}${after ?? ""}` || /%7b/iu.test(actualSegment)) {
      pathSegments[actualIndex] = `${before ?? ""}${encodedValue}${after ?? ""}`;
    }
  }
  parsed.pathname = pathSegments.join("/");
  const absolute = /^[a-z][a-z\d+.-]*:/iu.test(value) || value.startsWith("//");
  return absolute ? parsed.toString() : `${parsed.pathname}${parsed.search}${parsed.hash}`;
}

function decodeURIComponentSafe(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function mergeRequestQuery(value: string, variables: Record<string, string> = {}): string {
  const entries: Array<[string, string]> = [];
  const removeNames = new Set<string>();
  const removePrefixes: string[] = [];
  if (!standaloneRequestMode.value) {
    for (const parameter of queryParameters.value) {
      const managed = queryNamesManagedByParameter(parameter);
      for (const name of managed.names) removeNames.add(name);
      removePrefixes.push(...managed.prefixes);
      if (requestEnabled.value[parameterKey(parameter)] === false) continue;
      const parameterValue = parameterInputValue(parameter);
      if (!parameterValue.trim()) continue;
      const serialized = serializeOperationParameter(parameter, parameterValue, variables);
      if (serialized.ok) entries.push(...serialized.value.queryEntries);
      else if (executionVariableResolver && variables === activeExecutionVariables) throw new Error(t("specification.openapiParameterSerializationError"));
    }
  }
  for (const row of customQueries.value) {
    const name = row.name.trim();
    if (!name) continue;
    const parameter = customQueryParameter(row);
    const managed = queryNamesManagedByParameter(parameter);
    for (const queryName of managed.names) removeNames.add(queryName);
    removePrefixes.push(...managed.prefixes);
    if (row.enabled) {
      const serialized = serializeOperationParameter(parameter, row.value, variables);
      if (serialized.ok) entries.push(...serialized.value.queryEntries);
      else if (executionVariableResolver && variables === activeExecutionVariables) throw new Error(t("specification.openapiParameterSerializationError"));
    }
  }
  return mergeSerializedQuery(value, entries, { removeNames, removePrefixes, replace: replaceExplicitQuery.value });
}

function customQueryParameter(row: WorkbenchRow): OpenApiObject {
  return { name: row.name.trim(), in: "query", schema: row.schema ?? debugFieldSchema(row.type || "string", document.value), ...(row.style ? { style: row.style } : {}), ...(row.explode !== undefined ? { explode: row.explode } : {}), "x-specx-custom-value": row.value };
}

function mergeSerializedQuery(value: string, additions: Array<[string, string]>, options: { removeNames?: Set<string>; removePrefixes?: string[]; replace?: boolean } = {}): string {
  const hashIndex = value.indexOf("#");
  const hash = hashIndex >= 0 ? value.slice(hashIndex) : "";
  const withoutHash = hashIndex >= 0 ? value.slice(0, hashIndex) : value;
  const queryIndex = withoutHash.indexOf("?");
  const base = queryIndex >= 0 ? withoutHash.slice(0, queryIndex) : withoutHash;
  const query = queryIndex >= 0 ? withoutHash.slice(queryIndex + 1) : "";
  const removeNames = options.removeNames ?? new Set<string>();
  const removePrefixes = options.removePrefixes ?? [];
  const retained = options.replace
    ? []
    : query
        .split("&")
        .filter(Boolean)
        .filter((entry) => {
          const separator = entry.indexOf("=");
          const encodedName = separator < 0 ? entry : entry.slice(0, separator);
          let name = encodedName;
          try {
            name = decodeURIComponent(encodedName.replace(/\+/gu, "%20"));
          } catch {
            // Preserve malformed user-entered URL components as-is.
          }
          return !removeNames.has(name) && !removePrefixes.some((prefix) => name.startsWith(prefix));
        });
  const merged = [...retained, ...additions.map(([name, item]) => `${name}=${item}`)].join("&");
  return `${base}${merged ? `?${merged}` : ""}${hash}`;
}

function setRequestUrl(value: string) {
  requestUrlError.value = "";
  requestUrlOverride.value = value;
  standaloneRequestMode.value = standaloneRequestMode.value || !focusedEntry.value;
  replaceExplicitQuery.value = false;
  let parsed: URL;
  try {
    parsed = new URL(value, window.location.origin);
  } catch {
    return;
  }
  hydrateOperationParametersFromUrl(parsed, !standaloneRequestMode.value);
}

function restoreHistory(entry: WorkbenchHistoryEntry) {
  commitPendingRequestDraft();
  requestFromEntry(entry.request);
  showHistory.value = false;
}

function getScriptVariables(variables: Record<string, string>): Record<string, string> {
  const sensitiveKeys = new Set(requestEnvironmentVariables.value.filter((variable) => variable.sensitive).map((variable) => variable.key.toLowerCase()));
  return scriptVariables(Object.fromEntries(Object.entries(variables).filter(([key]) => !sensitiveKeys.has(key.toLowerCase()))));
}

function scriptContext(variables: Record<string, string>): Pick<OpenApiScriptInput, "variables" | "localVariables" | "scopes" | "info" | "messages"> {
  const safeVariables = getScriptVariables(variables);
  const globals = getScriptVariables(globalRequestVariables.value);
  const environmentKeys = new Set([...Object.keys(serviceRequestVariables.value), ...Object.keys(serviceLocalValues.value)]);
  return {
    variables: safeVariables,
    localVariables: executionScriptLocals,
    scopes: executionScriptScopes ?? { globals, environment: Object.fromEntries(Object.entries(safeVariables).filter(([key]) => environmentKeys.has(key))) },
    info: { requestName: String(focusedEntry.value?.operation.summary || currentRequest.value.url), requestId: focusedEntry.value?.identityKey || "", baseUrl: resolvedServerUrl.value },
    messages: { requestAlreadySent: translate("specification.openapiScriptRequestAlreadySent") },
  };
}

function applyScriptOutput(result: OpenApiScriptOutput, variables: Record<string, string>) {
  const before = scriptContext(variables).scopes;
  executionScriptLocals = result.localVariables;
  executionScriptScopes = result.scopes;
  for (const key of Object.keys(getScriptVariables(variables))) delete variables[key];
  Object.assign(variables, result.variables);
  scriptLogs.value = [...scriptLogs.value, ...result.logs];
  scriptTests.value = [...scriptTests.value, ...result.tests];
  const persist = (scope: string, values: Record<string, string>, initial: Record<string, string>, existing: Record<string, string>, definitions: Array<{ key: string; sensitive: boolean }>) => {
    const overrides = { ...existing };
    for (const key of Object.keys(initial)) if (!Object.hasOwn(values, key)) delete overrides[key];
    const known = new Set(definitions.map((row) => row.key));
    for (const [key, value] of Object.entries(values)) if (known.has(key) && value !== initial[key]) overrides[key] = value;
    localEnvironmentValues?.replaceScriptValues(scope, Object.fromEntries(Object.entries(values).filter(([key]) => !known.has(key))));
    const sensitive = new Set(definitions.filter((row) => row.sensitive).map((row) => row.key));
    localEnvironmentValues?.replace(
      scope,
      Object.entries(overrides).map(([key, localValue]) => ({ key, localValue, enabled: true, sensitive: sensitive.has(key) })),
    );
  };
  if (!props.publicTryIt && result.scopes && before && workspace?.catalog) {
    if (selectedEnvironment.value) persist(`${workspace.catalog.workspace.id}:${props.serviceId}:${selectedEnvironment.value.id}`, result.scopes.environment, before.environment, serviceLocalValues.value, selectedEnvironment.value.variables);
    if (environmentContext?.activeGlobal) persist(`${workspace.catalog.workspace.id}:global:${environmentContext.activeGlobal.id}`, result.scopes.globals, before.globals, globalLocalValues.value, environmentContext.activeGlobal.variables);
  }
}

function scriptHttpHost() {
  return createOpenApiScriptHttpHost({
    proxy: effectiveProxy.value,
    workspaceId: workspace?.catalog?.workspace.id,
    serviceId: props.serviceId,
    baseUrl: resolvedServerUrl.value || window.location.href,
    timeoutMs: Math.max(1000, Math.min(300000, Number(requestTimeoutSeconds.value) * 1000 || 20000)),
    followRedirects: followRedirects.value,
  });
}

async function executePreRequestScript(request: WorkbenchRequest, variables: Record<string, string>, signal: AbortSignal): Promise<WorkbenchRequest> {
  if (!preRequestScripts.value.some((script) => script.trim())) return JSON.parse(JSON.stringify(request)) as WorkbenchRequest;
  const result = await runOpenApiScriptSequence({ kind: "preRequest", scripts: preRequestScripts.value, request, ...scriptContext(variables) }, signal, scriptHttpHost());
  applyScriptOutput(result, variables);
  return result.request;
}

function deleteHistoryEntry(id: string) {
  if (!props.serviceId) return;
  historyEntries.value = removeWorkbenchHistory(props.serviceId, id).filter((entry) => entry.operationKey === props.focusOperationKey);
}

function clearHistory() {
  if (!props.serviceId) return;
  clearWorkbenchHistory(props.serviceId);
  historyEntries.value = [];
}

function importCurl() {
  const parsed = curlImportPreview.value;
  if (!parsed || "error" in parsed) {
    curlImportError.value = curlPreviewError.value || t("specification.openapiCurlInvalid");
    return;
  }
  if (!isSafeRequestUrl(parsed.request.url)) {
    curlImportError.value = t("specification.openapiUnsafeServer");
    return;
  }
  requestFromEntry(parsed.request);
  importedBinaryBody.value = Boolean(parsed.bodyFile);
  if (parsed.formFiles.length) {
    formRows.value.push(...parsed.formFiles.map((file) => ({ name: file.name ?? "file", value: "", enabled: true, isFile: true, required: true, files: [], uploadName: file.fileName, contentType: file.contentType })));
  }
  activeDebugSection.value = parsed.request.body || parsed.formFiles.length || parsed.bodyFile ? "body" : "params";
  showCurlImport.value = false;
  curlImportText.value = "";
  curlImportError.value = "";
}

const responseSchema = computed(() => {
  const status = responsePreview.value?.status;
  const exact = status === undefined ? null : responses.value.find(([key]) => key === String(status));
  const range = status === undefined ? null : responses.value.find(([key]) => /^([1-5])XX$/i.test(key) && key[0] === String(status)[0]);
  const fallback = responses.value.find(([key]) => key === "default") ?? (status === undefined ? responses.value.find(([key]) => key.startsWith("2")) : undefined);
  const selected = responses.value.find(([key]) => key === selectedResponseDefinition.value);
  const response = record((selected ?? exact ?? range ?? fallback)?.[1]);
  if (!response || !isRecord(response.content)) return null;
  const contentType = responsePreview.value?.contentType.split(";")[0]?.trim().toLowerCase();
  const media = contentType ? (record(response.content[contentType]) ?? record(response.content[`${contentType.split("/")[0]}/*`]) ?? record(response.content["*/*"])) : (record(response.content["application/json"]) ?? record(Object.values(response.content)[0]));
  return media?.schema ?? null;
});

const responseExampleTarget = computed(() => {
  const result = responsePreview.value;
  const operation = focusedEntry.value?.operation;
  const version = document.value?.openapi;
  if (!props.editable || !props.serviceId || !operation || !executedRequest.value?.operationExampleEligible || !result?.body || result.truncated || result.receivedBytes > 32 * 1024 || typeof version !== "string" || !/^3\.[01]\./.test(version)) return null;
  const target = responseCaptureTarget(operation.responses, String(result.status), result.contentType);
  return target && { ...target, version: target.canInfer ? (version.startsWith("3.1.") ? ("3.1" as const) : ("3.0" as const)) : undefined };
});

watch([responsePreview, () => props.content, () => props.focusOperationKey, () => props.editable], () => {
  showResponseExample.value = false;
  capturedResponseExample.value = null;
});

function closeResponseExample() {
  showResponseExample.value = false;
  void nextTick(() => extractExampleButton.value?.focus());
}

function stageResponseExample(input: ResponseExampleInput, infer: boolean) {
  const target = responseExampleTarget.value;
  if (!target || !props.focusOperationKey) return;
  capturedResponseExample.value = { ...input, status: target.status, mediaType: target.mediaType, action: target.action, infer, operationKey: props.focusOperationKey };
  showResponseExample.value = false;
  activeOperationView.value = "design";
  void nextTick(() => globalThis.document.getElementById("openapi-operation-design-tab")?.focus());
}

function schemaValidationMessages(result: OpenApiSchemaValidationResult): string[] {
  if (result.failure) return [t(result.failure === "schema" ? "specification.openapiValidationSchemaUnavailable" : result.failure === "timeout" ? "specification.openapiValidationTimeout" : "specification.openapiValidationUnavailable")];
  return result.issues.map((issue) => {
    if (issue.property !== undefined) return t(issue.keyword === "required" ? "specification.openapiValidationRequired" : "specification.openapiValidationAdditionalProperty", { path: issue.path, property: issue.property });
    if (issue.type) return t("specification.openapiValidationType", { path: issue.path, type: issue.type });
    if (issue.limit !== undefined) return t("specification.openapiValidationLimit", { path: issue.path, rule: issue.keyword, limit: issue.limit });
    if (issue.format) return t("specification.openapiValidationFormat", { path: issue.path, format: issue.format });
    return t("specification.openapiValidationRule", { path: issue.path, rule: issue.keyword });
  });
}

watch([responsePreview, selectedResponseDefinition, autoValidateResponse, () => props.content, () => props.focusOperationKey], () => {
  responseValidationAbort?.abort();
  responseValidationAbort = null;
  validatingResponse.value = false;
  contractResult.value = null;
  if (autoValidateResponse.value && responsePreview.value) void nextTick(validateResponseContract);
});
onBeforeUnmount(() => {
  clearRequestDraftTimer();
  oauthContextRevision += 1;
  activeOAuthPopup?.close();
  responseValidationAbort?.abort();
  cancelRequest.value?.abort();
  cancelRequest.value = null;
});

async function validateResponseContract() {
  if (!responsePreview.value || responseSchema.value === null || validatingResponse.value) return;
  const response = responsePreview.value;
  if (response.truncated) {
    contractResult.value = { ok: false, errors: [t("specification.openapiValidationTruncated")] };
    return;
  }
  const controller = new AbortController();
  responseValidationAbort = controller;
  validatingResponse.value = true;
  try {
    const parsed: unknown = response.contentType.includes("json") ? JSON.parse(response.body) : response.body;
    const result = await validateOpenApiSchemaInWorker({ document: document.value, schema: responseSchema.value, body: parsed }, controller.signal);
    if (!controller.signal.aborted && responsePreview.value === response) contractResult.value = { ok: result.valid, errors: schemaValidationMessages(result) };
  } catch {
    if (!controller.signal.aborted && responsePreview.value === response) contractResult.value = { ok: false, errors: [t("specification.openapiResponseInvalidJson")] };
  } finally {
    if (responseValidationAbort === controller) {
      validatingResponse.value = false;
      responseValidationAbort = null;
    }
  }
}

watch(responseTab, () => {
  responseCopyStatus.value = "";
});

async function copyResponse(text: string) {
  if (!responsePreview.value) return;
  try {
    await navigator.clipboard.writeText(text);
    responseCopyStatus.value = t("specification.openapiCopied");
  } catch {
    responseCopyStatus.value = t("specification.openapiCopyFailed");
  }
}

function downloadResponse() {
  const response = responsePreview.value;
  if (!response) return;
  const type = response.contentType || "application/octet-stream";
  const bytes = response.rawBytes ? Uint8Array.from(response.rawBytes) : new TextEncoder().encode(response.body);
  const href = URL.createObjectURL(new Blob([bytes], { type }));
  const link = globalThis.document.createElement("a");
  link.href = href;
  const operationId = focusedEntry.value?.operation.operationId;
  const name = typeof operationId === "string" && operationId ? operationId : "openapi";
  const subtype = type.split(";", 1)[0]!.split("/")[1] || "bin";
  const extension = subtype.includes("json") ? "json" : subtype.includes("xml") ? "xml" : subtype === "plain" ? "txt" : subtype === "octet-stream" ? "bin" : subtype.replace(/[^a-z0-9]/giu, "");
  link.download = response.fileName || `${name}-response.${extension || "bin"}`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(href), 0);
}

function formatByteSize(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function documentCookie(pair: string) {
  window.document.cookie = `${pair}; Path=/; SameSite=Lax`;
}

async function runRequest() {
  const operation = focusedEntry.value;
  if (!operation || (!props.serviceId && !props.publicTryIt) || running.value) return;
  requestError.value = "";
  requestErrorPhase.value = null;
  scriptLogs.value = [];
  scriptTests.value = [];
  executionScriptLocals = undefined;
  executionScriptScopes = undefined;
  scriptError.value = "";
  requestUrlError.value = "";
  const candidateUrl = expand(currentRequestUrl.value, requestExecutionVariables.value).trim();
  if (!candidateUrl || (!resolvedServerUrl.value && !isAbsoluteHttpUrl(candidateUrl))) {
    requestUrlError.value = props.publicTryIt ? t("openapiPublication.requestUrlRequired") : t("specification.openapiNoServer");
    await nextTick();
    requestTarget.value?.focusUrl();
    return;
  }
  if ((props.publicTryIt && !isAbsoluteHttpUrl(candidateUrl)) || !isSafeRequestUrl(candidateUrl)) {
    requestUrlError.value = props.publicTryIt ? t("openapiPublication.requestUrlRequired") : t("specification.openapiUnsafeServer");
    await nextTick();
    requestTarget.value?.focusUrl();
    return;
  }
  if (!standaloneRequestMode.value && requestParameterSerializationErrors.value.length) {
    requestError.value = t("specification.openapiParameterSerializationError");
    return;
  }
  if (!requestBodyJsonValid.value) {
    requestError.value = t("specification.openapiInvalidJsonBody");
    activeDebugSection.value = "body";
    return;
  }
  if (binaryRequestBody.value && (importedBinaryBody.value || requestBody.value?.required === true) && !requestFile.value) {
    requestError.value = t("specification.openapiRequiredRequestFile");
    return;
  }
  let disabledHeaders = computedHeaderRows.value.filter((row) => !row.enabled).map((row) => row.name.toLowerCase());
  const authConfig = currentAuthConfig.value;
  const authTokens = executionAuthTokens();
  running.value = true;
  requestError.value = "";
  responsePreview.value = null;
  executedRequest.value = null;
  responseTab.value = "body";
  responseCopyStatus.value = "";
  const controller = new AbortController();
  cancelRequest.value = controller;
  const timeoutSeconds = Math.max(1, Math.min(300, Number(requestTimeoutSeconds.value) || 20));
  const timeout = window.setTimeout(() => controller.abort(), timeoutSeconds * 1000);
  let authIssue: RequestAuthenticationError | null = null;
  let timingCapture: ReturnType<typeof observeResponseResourceTiming> | undefined;
  const preparedAt = performance.now();
  const environment = selectedEnvironment.value;
  const environmentServerId = selectedEnvironmentServer.value?.id;
  const useEnvironmentPrefix = showEnvironmentPrefix.value;
  const requestPath = requestPathTemplate.value;
  const originalRequest = currentRequest.value;
  const requestProxy = effectiveProxy.value;
  const localValuesAtSend = serviceLocalValues.value;
  const dynamicUpdates: Record<string, string> = {};
  executionVariableResolver = createOpenApiVariableResolver({
    onVariableUpdate: (name, value) => {
      dynamicUpdates[name] = value;
    },
  });
  try {
    let variables: Record<string, string> = { ...requestExecutionVariables.value };
    let runtimeBaseServer = resolvedServerUrl.value;
    if (environment && props.serviceId && workspace) {
      const runtime = await workspace.resolveOpenApiEnvironment(props.serviceId, environment.id);
      if (!runtime) throw new Error(t("specification.openapiRequestFailed"));
      const runtimeServer = runtime.servers?.find((server) => server.id === environmentServerId);
      if (environmentServerId && !runtimeServer) throw new Error(t("environmentContext.serverMissing"));
      runtimeBaseServer = runtimeServer?.baseUrl ?? runtime.baseUrl;
      variables = {
        ...variables,
        ...resolveEnvironmentVariableValues(
          runtime.variables.map((item) => ({ ...item, enabled: true, sensitive: false })),
          localValuesAtSend,
        ),
      };
    }
    if (controller.signal.aborted || cancelRequest.value !== controller) return;
    activeExecutionVariables = variables;
    const scriptRequest = preRequestScripts.value.some((script) => script.trim())
      ? {
          ...originalRequest,
          headers: [
            ...originalRequest.headers,
            ...computedHeaderRows.value
              .filter((row) => ["accept", "user-agent", "content-type"].includes(row.name.toLowerCase()) && !row.value.includes("…") && !currentRequest.value.headers.some((header) => header.name.toLowerCase() === row.name.toLowerCase()))
              .map(({ name, value, enabled }) => ({ name, value, enabled })),
          ],
        }
      : originalRequest;
    const executionRequest = await executePreRequestScript(scriptRequest, variables, controller.signal);
    disabledHeaders = [...new Set([...disabledHeaders, ...(executionRequest.disabledHeaders ?? [])])].filter((name) => !executionRequest.headers.some((row) => row.enabled && row.name.toLowerCase() === name));
    if (controller.signal.aborted || cancelRequest.value !== controller) return;
    const operationPath = typeof operation.path === "string" ? operation.path : String(operation.path);
    const hasExplicitUrl = Boolean(requestUrlOverride.value);
    const scriptChangedUrl = executionRequest.url !== originalRequest.url;
    const expandedBase = expand(expandOpenApiServerUrl(document.value, runtimeBaseServer, variables, operation), variables);
    let url: string;
    if (useEnvironmentPrefix) {
      const target = mergeRequestPath(expand(scriptChangedUrl ? executionRequest.url : requestPath, variables), variables);
      const withQuery = scriptChangedUrl || standaloneRequestMode.value ? target : mergeRequestQuery(target, variables);
      url = resolveStandaloneUrl(prefixOpenApiRequestPath(expandedBase, withQuery), runtimeBaseServer, variables);
    } else if (scriptChangedUrl || standaloneRequestMode.value) {
      url = resolveStandaloneUrl(mergeRequestPath(executionRequest.url, variables), runtimeBaseServer, variables);
    } else {
      const requestBase = hasExplicitUrl ? executionRequest.url : prefixOpenApiRequestPath(expandedBase, expand(operationPath, variables));
      url = resolveStandaloneUrl(mergeRequestQuery(mergeRequestPath(requestBase, variables), variables), runtimeBaseServer, variables);
    }
    if (props.publicTryIt && (!isAbsoluteHttpUrl(url) || /\{\{|<</u.test(url))) throw new Error(t("openapiPublication.requestUrlRequired"));
    let headers = new Headers();
    const operationHeaderNames = new Set(standaloneRequestMode.value ? [] : headerParameters.value.map((parameter) => String(parameter.name).toLowerCase()));
    if (!standaloneRequestMode.value) {
      for (const parameter of headerParameters.value) {
        const name = String(parameter.name);
        const row = executionRequest.headers.find((header) => header.name.toLowerCase() === name.toLowerCase());
        if (!row?.enabled || !row.value.trim()) continue;
        const serialized = serializeOperationParameter(parameter, row.value, variables);
        if (!serialized.ok) throw new Error(t("specification.openapiParameterSerializationError"));
        headers.set(name, serialized.value.headerValue);
      }
    }
    for (const header of executionRequest.headers) {
      if (operationHeaderNames.has(header.name.toLowerCase())) continue;
      if (header.enabled && header.name.trim() && header.value.trim()) headers.set(header.name.trim(), expand(header.value, variables));
    }
    const operationCookieNames = new Set(standaloneRequestMode.value ? [] : cookieParameters.value.map((parameter) => String(parameter.name)));
    const cookieEntries: Array<[string, string]> = [];
    if (!standaloneRequestMode.value) {
      for (const parameter of cookieParameters.value) {
        const name = String(parameter.name);
        const row = executionRequest.cookies.find((cookie) => cookie.name === name);
        if (!row?.enabled || !row.value.trim()) continue;
        const serialized = serializeOperationParameter(parameter, row.value, variables);
        if (!serialized.ok) throw new Error(t("specification.openapiParameterSerializationError"));
        cookieEntries.push(...serialized.value.cookieEntries.map(([cookieName, cookieValue]): [string, string] => [cookieName, expand(cookieValue, variables)]));
      }
    }
    for (const cookie of executionRequest.cookies) {
      if (operationCookieNames.has(cookie.name)) continue;
      if (cookie.enabled && cookie.name.trim()) cookieEntries.push([cookie.name.trim(), standaloneRequestMode.value ? expand(cookie.value, variables) : encodeURIComponent(expand(cookie.value, variables))]);
    }
    const cookieHeader = cookieEntries.map(([name, value]) => `${name}=${value}`).join("; ");
    if (cookieHeader) headers.set("cookie", cookieHeader);
    const authentication = applySecurity(headers, url, variables, authConfig, standaloneRequestMode.value ? undefined : operation.operation, authTokens, disabledHeaders);
    url = authentication.url;
    headers = authentication.headers;
    if (!isSafeRequestUrl(url)) throw new Error(t("specification.openapiUnsafeServer"));
    const executionForm = executionRequest.form?.map((row) => ({ ...formRows.value.find((original) => original.name === row.name), ...row })) ?? formRows.value;
    const executionMediaType = executionRequest.mediaType;
    const missingFile = executionMediaType === "multipart/form-data" ? executionForm.find((row) => row.enabled && row.isFile && row.required && !row.files?.length) : undefined;
    if (missingFile) throw new Error(t("specification.openapiRequiredFile", { name: missingFile.name }));
    let body: BodyInit | undefined;
    if (binaryRequestBody.value) {
      body = requestFile.value ?? undefined;
      if (body && executionMediaType && !headers.has("content-type")) headers.set("content-type", executionMediaType);
    } else if (isFormMediaType(executionMediaType)) {
      if (executionMediaType === "multipart/form-data") {
        headers.delete("content-type");
        const form = new FormData();
        for (const row of executionForm) {
          if (!row.enabled || !row.name.trim()) continue;
          if (row.isFile) {
            for (const file of row.files ?? []) form.append(row.name.trim(), row.contentType ? new Blob([file], { type: row.contentType }) : file, row.uploadName || file.name);
          } else {
            form.append(row.name.trim(), expandOpenApiParameterValue(row.value, row.schema ?? debugFieldSchema(row.type || "string", document.value), document.value, executionVariableResolver!, variables));
          }
        }
        body = form;
      } else {
        const params = new URLSearchParams();
        for (const row of executionForm) if (row.enabled && row.name.trim()) params.append(row.name.trim(), expandOpenApiParameterValue(row.value, row.schema ?? debugFieldSchema(row.type || "string", document.value), document.value, executionVariableResolver!, variables));
        body = params;
        if (!headers.has("content-type")) headers.set("content-type", executionMediaType);
      }
    } else if (executionRequest.body.trim()) {
      body = executionMediaType.includes("json") ? executionVariableResolver!.expandJson(executionRequest.body, variables) : expand(executionRequest.body, variables);
      if (executionMediaType.includes("json") && String(body).trim()) {
        try {
          JSON.parse(String(body));
        } catch {
          throw new Error(t("specification.openapiInvalidJsonBody"));
        }
      }
      if (executionMediaType && !headers.has("content-type")) headers.set("content-type", executionMediaType);
    }
    for (const name of disabledHeaders) headers.delete(name);
    if (requestProxy === "browser" && body !== undefined && disabledHeaders.includes("content-type")) {
      // Bytes prevent fetch from silently recreating a disabled Content-Type.
      body = new Uint8Array(await new Request(url, { method: executionRequest.method, body }).arrayBuffer());
    }
    if (requestProxy === "browser" && headers.has("cookie")) {
      if (new URL(url, window.location.origin).origin !== window.location.origin) throw new Error(t("specification.openapiCrossOriginCookies"));
      for (const pair of headers.get("cookie")!.split(";")) documentCookie(pair.trim());
      headers.delete("cookie");
    }
    if (Object.keys(dynamicUpdates).length) {
      const context = scriptContext(variables);
      const scopes = context.scopes!;
      Object.assign(scopes.environment, dynamicUpdates);
      applyScriptOutput({ request: executionRequest, variables: getScriptVariables(variables), localVariables: executionScriptLocals, scopes, logs: [], tests: [] }, variables);
    }
    const started = performance.now();
    const method = executionRequest.method.toUpperCase();
    const operationExampleEligible = !standaloneRequestMode.value && method === operation.method.toUpperCase();
    const submittedHeaders: Array<[string, string]> = [...headers.entries()];
    const bodyBytes = requestBodyByteSize(body);
    executedRequest.value = { method, url, headers: submittedHeaders, body: describeRequestBody(body), mediaType: executionMediaType, operationExampleEligible };
    let agent: AgentResponse | undefined;
    let response: Response;
    let headersAt: number;
    if (requestProxy === "agent" && workspace?.catalog && props.serviceId) {
      const input = await serializeAgentRequest({ method, url: new URL(url, window.location.origin).href, headers, body, disabledHeaders, timeoutMs: timeoutSeconds * 1000, followRedirects: followRedirects.value });
      agent = await executeAgentRequest(workspace.catalog.workspace.id, props.serviceId, input, controller.signal);
      const bytes = base64ToBytes(agent.bodyBase64);
      response = new Response([101, 204, 205, 304].includes(agent.status) ? null : bytes.slice().buffer, { status: agent.status, statusText: agent.statusText, headers: agent.headers });
      headersAt = started + agent.timing.totalMs - agent.timing.downloadMs;
      executedRequest.value = { ...describeExecutedRequest({ method: agent.requestMethod, url: agent.url, headers: agent.requestHeaders, body: base64ToBytes(agent.requestBodyBase64) }), operationExampleEligible };
    } else {
      timingCapture = observeResponseResourceTiming(url, started);
      response = await fetch(url, { method, headers, body, signal: controller.signal, credentials: props.publicTryIt || disabledHeaders.includes("cookie") ? "omit" : "same-origin", redirect: followRedirects.value ? "follow" : "error" });
      headersAt = performance.now();
    }
    const payload = await readResponse(response);
    if (agent?.truncated) payload.truncated = true;
    const receivedAt = performance.now();
    if (controller.signal.aborted || cancelRequest.value !== controller) return;
    const responseHeaders: Array<[string, string]> = agent?.headers ?? [...response.headers.entries()];
    const duration = Math.round(agent?.timing.totalMs ?? receivedAt - started);
    let scriptResponse: WorkbenchResponse = { status: response.status, statusText: response.statusText, duration, headers: responseHeaders, scriptLogs: scriptLogs.value, ...payload };
    let visualize: string | undefined;
    if (postResponseScripts.value.some((script) => script.trim())) {
      try {
        const actual = executedRequest.value;
        const sentRequest = actual ? { ...executionRequest, url: actual.url, method: actual.method, body: actual.body, headers: actual.headers.map(([name, value]) => ({ name, value, enabled: true })) } : executionRequest;
        const result = await runOpenApiScriptSequence({ kind: "postResponse", scripts: postResponseScripts.value, request: sentRequest, response: scriptResponse, ...scriptContext(variables) }, controller.signal, scriptHttpHost());
        applyScriptOutput(result, variables);
        if (result.response) scriptResponse = result.response;
        if (result.visualization) visualize = result.visualization;
        scriptError.value = "";
        scriptResponse = { ...scriptResponse, scriptLogs: scriptLogs.value };
      } catch (cause) {
        if (cause instanceof OpenApiScriptError && cause.output) {
          scriptLogs.value = [...scriptLogs.value, ...cause.output.logs];
          scriptTests.value = [...scriptTests.value, ...cause.output.tests];
        }
        scriptError.value = t("specification.openapiScriptStepError", { phase: t("specification.openapiDebugAfter"), step: cause instanceof OpenApiScriptError ? cause.step : 1, detail: cause instanceof Error ? cause.message : "script_failed" });
        scriptResponse = { ...scriptResponse, scriptError: scriptError.value, scriptLogs: scriptLogs.value };
      }
    }
    if (controller.signal.aborted || cancelRequest.value !== controller) return;
    const completedAt = performance.now();
    const resource = await timingCapture?.finish(receivedAt, response.url);
    if (controller.signal.aborted || cancelRequest.value !== controller) return;
    responsePreview.value = {
      status: scriptResponse.status,
      statusText: scriptResponse.statusText,
      duration: scriptResponse.duration,
      headers: scriptResponse.headers,
      contentType: response.headers.get("content-type") ?? "",
      scriptLogs: scriptResponse.scriptLogs,
      scriptError: scriptResponse.scriptError,
      agent,
      visualize,
      metrics: agent
        ? buildAgentResponseTelemetry(agent, payload.receivedBytes, payload.truncated)
        : buildResponseTelemetry({
            preparedAt,
            startedAt: started,
            headersAt,
            receivedAt,
            completedAt,
            request: { method, url, headers: submittedHeaders, bodyBytes },
            response: { status: response.status, statusText: response.statusText, url: response.url || url, headers: responseHeaders, receivedBytes: payload.receivedBytes, truncated: payload.truncated, redirected: response.redirected },
            resource,
          }),
      ...payload,
      ...(scriptResponse.body !== payload.body ? { body: scriptResponse.body, rawBytes: new TextEncoder().encode(scriptResponse.body) } : {}),
      scriptTests: scriptTests.value,
    };
    responseTab.value = "body";
    const responseDefinition = responses.value.find(([key]) => key === String(response.status)) ?? responses.value.find(([key]) => /^([1-5])XX$/i.test(key) && key[0] === String(response.status)[0]) ?? responses.value.find(([key]) => key === "default");
    if (responseDefinition) selectedResponseDefinition.value = responseDefinition[0];
    if (props.serviceId && props.focusOperationKey) {
      const safeHistoryRequest = redactAuthRequest({ ...executionRequest, url, method, headers: [...headers.entries()].map(([name, value]) => ({ name, value, enabled: true })) }, authentication.targets);
      historyEntries.value = appendWorkbenchHistory(props.serviceId, {
        operationKey: standaloneRequestMode.value ? "imported" : props.focusOperationKey,
        operationLabel: `${method} ${standaloneRequestMode.value ? safeHistoryRequest.url : operation.path}`,
        request: safeHistoryRequest,
        response: { status: response.status, statusText: response.statusText, duration },
      }).filter((entry) => entry.operationKey === (standaloneRequestMode.value ? "imported" : props.focusOperationKey));
    }
  } catch (cause) {
    if (cause instanceof RequestAuthenticationError && cancelRequest.value === controller && !controller.signal.aborted) {
      activeDebugSection.value = "auth";
      authIssue = cause;
    }
    if (cancelRequest.value === controller) {
      if (cause instanceof OpenApiScriptError && cause.output) {
        scriptLogs.value = [...scriptLogs.value, ...cause.output.logs];
        scriptTests.value = [...scriptTests.value, ...cause.output.tests];
      }
      const message = cause instanceof Error ? cause.message : "";
      requestErrorPhase.value = cause instanceof OpenApiScriptError ? "before" : null;
      requestError.value = controller.signal.aborted
        ? controller.signal.reason === "cancelled"
          ? t("specification.openapiRequestCancelled")
          : t("specification.openapiRequestTimeout", { seconds: timeoutSeconds })
        : cause instanceof OpenApiScriptError
          ? t("specification.openapiScriptStepError", { phase: t("specification.openapiDebugBefore"), step: cause.step, detail: cause.message })
          : cause instanceof OpenApiVariableError
            ? t(cause.reason === "cycle" ? "specification.openapiDynamic.cycle" : "specification.openapiDynamic.invalid")
            : message || t("specification.openapiRequestFailed");
    }
  } finally {
    executionVariableResolver = undefined;
    timingCapture?.disconnect();
    window.clearTimeout(timeout);
    if (cancelRequest.value === controller) {
      running.value = false;
      cancelRequest.value = null;
      if (authIssue) {
        const issue = authIssue;
        await nextTick();
        const firstField = authSchemes.value.find((item) => item.name === issue.scheme)?.scheme.scheme === "basic" ? "username" : "value";
        const name = authConfig.mode === "specification" ? `openapi-auth-${encodeURIComponent(issue.scheme)}-${firstField}` : `openapi-auth-${firstField}`;
        const panel = globalThis.document.getElementById("openapi-operation-debug");
        const control = [...(panel?.querySelectorAll<HTMLInputElement>("input") ?? [])].find((input) => input.name === name) ?? panel?.querySelector<HTMLSelectElement>('select[name="openapi-auth-scheme"]');
        control?.focus();
      }
    }
  }
}

function stopRequest() {
  cancelRequest.value?.abort("cancelled");
}

function record(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

function selectOperationView(value: string) {
  if (value === "docs" || value === "debug" || value === "design") activeOperationView.value = value;
}

function handleDebugAction(action: "undo" | "redo" | "reset" | "history" | "curl") {
  if (action === "undo") undoRequestDraft();
  else if (action === "redo") redoRequestDraft();
  else if (action === "reset") resetRequest();
  else toggleWorkbenchDrawer(action);
}
function debugParameterRows(section: "path" | "query" | "header" | "cookie"): DebugParameter[] {
  if (standaloneRequestMode.value && section !== "path") return [];
  return requestParameters.value
    .filter((parameter) => parameter.in === section)
    .map((parameter) => ({
      key: parameterKey(parameter),
      name: String(parameter.name),
      type: parameterTypeLabel(parameter),
      description: parameterDescription(parameter),
      schema: parameterSchema(parameter),
      style: typeof parameter.style === "string" ? parameter.style : undefined,
      explode: typeof parameter.explode === "boolean" ? parameter.explode : undefined,
      placeholder: placeholderFor(parameter),
      value: parameterInputValue(parameter),
      enabled: requestEnabled.value[parameterKey(parameter)] !== false,
      required: parameter.required === true,
    }));
}
function setDebugParameterConfiguration(key: string, value: OpenApiParameterConfiguration) {
  parameterMetadata.value = { ...parameterMetadata.value, [key]: { ...parameterMetadata.value[key], ...value } };
}
function setDebugParameterMetadata(key: string, field: "type" | "description" | "name", value: string) {
  parameterMetadata.value = { ...parameterMetadata.value, [key]: { ...parameterMetadata.value[key], [field]: value, ...(field === "type" ? { schema: undefined } : {}) } };
}
function setDebugParameterValue(key: string, value: string) {
  requestValues.value[key] = value;
}
function setDebugParameterEnabled(key: string, enabled: boolean) {
  if (key.startsWith("generated:")) generatedHeaderEnabled.value = { ...generatedHeaderEnabled.value, [key.slice(10)]: enabled };
  else requestEnabled.value[key] = enabled;
}

function selectDebugSection(value: string) {
  if (["params", "body", "headers", "cookies", "auth", "before", "after", "settings"].includes(value)) activeDebugSection.value = value as DebugSection;
}

function toggleWorkbenchDrawer(target: "history" | "curl") {
  const shouldOpen = target === "history" ? !showHistory.value : !showCurlImport.value;
  showHistory.value = shouldOpen && target === "history";
  showCurlImport.value = shouldOpen && target === "curl";
}
</script>

<template>
  <article v-if="props.focusOperationKey" class="openapi-operation-page" :aria-label="t('specification.openapiOperation')" @keydown.capture="handleRequestDraftKeydown">
    <template v-if="focusedEntry">
      <OpenApiOperationTabs v-if="!props.embeddedWorkbench && (!props.reader || props.publicTryIt)" :model-value="activeOperationView" :tabs="operationTabs" :ariaLabel="t('specification.openapiOperationViews')" @update:model-value="selectOperationView" />

      <section
        v-if="activeOperationView === 'docs'"
        id="openapi-operation-documentation"
        class="openapi-documentation-view"
        :role="!props.reader || props.publicTryIt ? 'tabpanel' : undefined"
        :aria-labelledby="!props.reader || props.publicTryIt ? 'openapi-operation-documentation-tab' : undefined"
        :tabindex="!props.reader || props.publicTryIt ? 0 : undefined"
      >
        <OpenApiOperationDocument
          :key="props.content + props.focusOperationKey"
          :document="document"
          :entry="focusedEntry"
          :specification-title="props.specificationTitle"
          :specification-version="props.specificationVersion"
          :show-operation-id="props.showOperationId"
          :schema-display="props.schemaDisplay"
          :navigate-model="props.navigateModel"
          :can-debug="Boolean(props.serviceId || props.publicTryIt)"
          :service-id="props.publicTryIt ? undefined : props.serviceId"
          @debug="selectOperationView('debug')"
        />
      </section>

      <section
        v-else-if="activeOperationView === 'debug' && (props.serviceId || props.publicTryIt)"
        id="openapi-operation-debug"
        class="openapi-debug-workbench"
        :role="props.embeddedWorkbench ? undefined : 'tabpanel'"
        :aria-labelledby="props.embeddedWorkbench ? undefined : 'openapi-operation-debug-tab'"
        :tabindex="props.embeddedWorkbench ? undefined : 0"
      >
        <OpenApiRequestTarget
          ref="requestTarget"
          :method="requestMethodEditor"
          :url="requestUrlEditor"
          url-editable
          :method-editable="!props.publicTryIt"
          :url-error="requestUrlError"
          :running="running"
          :public-try-it="props.publicTryIt"
          :unsafe-server-url="unsafeServerUrl"
          @update:url="requestUrlEditor = $event"
          @update:method="requestMethodOverride = $event"
          @send="runRequest"
          @cancel="stopRequest"
        >
          <template v-if="showEnvironmentPrefix" #prefix>
            <OpenApiRequestEnvironment :choices="environmentChoices" :environment-id="selectedEnvironment?.id ?? null" :server-id="selectedEnvironmentServer?.id ?? null" :base-url="previewServerUrl" :ignored="ignoresEnvironmentPrefix" :disabled="running" @select="selectRequestEnvironment" />
          </template>
        </OpenApiRequestTarget>
        <section v-if="showHistory && !props.publicTryIt" class="workbench-drawer" :aria-label="t('specification.openapiHistory')">
          <div class="workbench-drawer-heading">
            <h4>{{ t("specification.openapiHistory") }}</h4>
            <div class="flex items-center gap-2">
              <button v-if="historyEntries.length" type="button" class="text-xs text-destructive hover:underline" @click="clearHistory">{{ t("specification.openapiClearHistory") }}</button
              ><button type="button" class="icon-button" :aria-label="t('common.close')" @click="showHistory = false"><X class="h-3.5 w-3.5" /></button>
            </div>
          </div>
          <div v-if="historyEntries.length" class="workbench-entry-list">
            <div v-for="entry in historyEntries" :key="entry.id" class="workbench-entry">
              <button type="button" class="workbench-entry-main" @click="restoreHistory(entry)">
                <strong>{{ entry.operationLabel }}</strong
                ><span>{{ formatDate(entry.createdAt) }} · {{ entry.response?.status ?? t("specification.openapiNotSent") }}</span></button
              ><button type="button" class="icon-button" :aria-label="t('common.delete')" @click="deleteHistoryEntry(entry.id)"><Trash2 class="h-3.5 w-3.5" /></button>
            </div>
          </div>
          <p v-else class="workbench-empty">{{ t("specification.openapiHistoryEmpty") }}</p>
        </section>
        <section v-if="showCurlImport && !props.publicTryIt" class="workbench-drawer" :aria-label="t('specification.openapiImportCurl')">
          <div class="workbench-drawer-heading">
            <h4>{{ t("specification.openapiImportCurl") }}</h4>
            <button type="button" class="icon-button" :aria-label="t('common.close')" @click="showCurlImport = false"><X class="h-3.5 w-3.5" /></button>
          </div>
          <label class="sr-only" for="openapi-curl-import">{{ t("specification.openapiCurlInput") }}</label
          ><textarea
            id="openapi-curl-import"
            v-model="curlImportText"
            class="field-control w-full min-h-32 resize-y font-mono text-xs"
            name="openapi-curl-import"
            :placeholder="t('specification.openapiCurlPlaceholder')"
            spellcheck="false"
            @input="curlImportError = ''"
            :aria-invalid="Boolean(curlPreviewError || curlImportError)"
            aria-describedby="openapi-curl-help openapi-curl-error"
          />
          <p id="openapi-curl-help" class="mt-2 text-xs text-muted-foreground">{{ t("specification.openapiCurlHelp") }}</p>
          <div v-if="curlImportPreview && 'request' in curlImportPreview" class="curl-import-preview" aria-live="polite">
            <div>
              <span class="method-chip" :data-method="curlImportPreview.request.method">{{ curlImportPreview.request.method }}</span
              ><code>{{ curlImportPreview.request.url }}</code>
            </div>
            <p>{{ t("specification.openapiCurlPreview", { headers: curlImportPreview.request.headers.length, cookies: curlImportPreview.request.cookies.length, mediaType: curlImportPreview.request.mediaType || t("specification.openapiEmptyRequestBody") }) }}</p>
            <p v-if="curlImportPreview.warnings.includes('files')">{{ t("specification.openapiCurlFilesWarning") }}</p>
            <p v-if="curlImportPreview.warnings.includes('network')">{{ t("specification.openapiCurlNetworkWarning") }}</p>
          </div>
          <p id="openapi-curl-error" v-if="curlImportError || curlPreviewError" class="mt-2 text-xs text-destructive" role="alert">{{ curlImportError || curlPreviewError }}</p>
          <div class="mt-2 flex justify-end">
            <Button size="sm" :disabled="!curlImportPreview || 'error' in curlImportPreview" @click="importCurl"><Upload class="h-3.5 w-3.5" />{{ t("specification.openapiApplyCurl") }}</Button>
          </div>
        </section>
        <OpenApiDebugSplitPane>
          <template #request>
            <section class="openapi-debug-section openapi-request-panel">
              <div class="openapi-debug-sections">
                <OpenApiOperationTabs :model-value="activeDebugSection" :tabs="debugSectionTabs" :ariaLabel="t('specification.openapiDebugSections')" @update:model-value="selectDebugSection" />
                <OpenApiDebugActions v-if="!props.publicTryIt" :can-undo="canUndoRequestDraft" :can-redo="canRedoRequestDraft" @action="handleDebugAction" />
              </div>
              <div id="openapi-debug-section-panel" class="openapi-debug-section-panel" role="tabpanel" :aria-labelledby="`openapi-debug-section-tab-${activeDebugSection}`" tabindex="0">
                <OpenApiDebugParameterSection
                  v-show="activeDebugSection === 'params'"
                  section="query"
                  :title="t('specification.openapiQueryParameters')"
                  :document="document"
                  :parameters="debugParameterRows('query')"
                  v-model:rows="customQueries"
                  @apply="applyBulkRows('query', $event)"
                  @configuration="setDebugParameterConfiguration"
                  @metadata="setDebugParameterMetadata"
                  @value="setDebugParameterValue"
                  @enabled="setDebugParameterEnabled"
                />
                <OpenApiDebugParameterSection
                  v-show="activeDebugSection === 'params'"
                  section="path"
                  :title="t('specification.openapiPathParameters')"
                  :document="document"
                  :parameters="debugParameterRows('path')"
                  v-if="pathParameters.length"
                  :editable="false"
                  @configuration="setDebugParameterConfiguration"
                  @metadata="setDebugParameterMetadata"
                  @value="setDebugParameterValue"
                  @enabled="setDebugParameterEnabled"
                />
                <OpenApiDebugParameterSection
                  v-show="activeDebugSection === 'headers'"
                  section="header"
                  :title="t('specification.openapiHeaders')"
                  :document="document"
                  :parameters="debugParameterRows('header')"
                  v-model:rows="customHeaders"
                  :computed-rows="computedHeaderRows"
                  @apply="applyBulkRows('header', $event)"
                  v-model:show-computed="showComputedHeaders"
                  @configuration="setDebugParameterConfiguration"
                  @metadata="setDebugParameterMetadata"
                  @value="setDebugParameterValue"
                  @enabled="setDebugParameterEnabled"
                />
                <p v-show="activeDebugSection === 'cookies' && effectiveProxy === 'browser'" class="text-xs text-muted-foreground">{{ t("specification.openapiCookieBrowserHelp") }}</p>
                <OpenApiDebugParameterSection
                  v-show="activeDebugSection === 'cookies'"
                  section="cookie"
                  :title="t('specification.openapiCookies')"
                  :document="document"
                  :parameters="debugParameterRows('cookie')"
                  v-model:rows="cookies"
                  @apply="applyBulkRows('cookie', $event)"
                  @configuration="setDebugParameterConfiguration"
                  @metadata="setDebugParameterMetadata"
                  @value="setDebugParameterValue"
                  @enabled="setDebugParameterEnabled"
                />
                <div v-show="activeDebugSection === 'auth'" class="debug-field-group debug-auth-panel">
                  <h5>{{ t("specification.openapiAuthorization") }}</h5>
                  <AppSelect v-model="selectedAuthScheme" size="form" :disabled="running || oauthBusy" class="field-control" name="openapi-auth-scheme" :aria-label="t('specification.openapiAuthorization')">
                    <option value="specification">{{ t("specification.openapiUseSpecificationAuth") }}</option>
                    <option value="none">{{ t("specification.openapiNoAuth") }}</option>
                    <option v-for="item in authSchemes" :key="item.name" :value="item.name">{{ item.name }}</option>
                  </AppSelect>
                  <OpenApiAuthRequirements
                    v-if="selectedAuthScheme === 'specification'"
                    v-model:alternative="authAlternative"
                    v-model:inputs="authInputs"
                    :requirements="standaloneRequestMode ? [] : security"
                    :schemes="authSchemes"
                    :variables="requestEnvironmentVariables"
                    :token-schemes="Object.keys(executionAuthTokens())"
                    :disabled="running || oauthBusy"
                    @authorize="selectedAuthScheme = $event"
                  />
                  <p v-else-if="selectedAuthScheme !== 'none'" class="openapi-auth-help">{{ t("specification.openapiAuth.persistence", { example: authTemplateExample }) }}</p>
                  <input
                    v-if="selectedAuthScheme !== 'specification' && selectedAuthScheme !== 'none' && authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.type === 'http' && authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.scheme === 'basic'"
                    v-model="authUsername"
                    :disabled="running || oauthBusy"
                    class="field-control mt-2"
                    type="text"
                    :placeholder="t('specification.openapiUsername')"
                    name="openapi-auth-username"
                    :aria-label="t('specification.openapiUsername')"
                    autocomplete="username"
                  />
                  <input
                    v-if="selectedAuthScheme !== 'specification' && selectedAuthScheme !== 'none' && authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.type === 'http' && authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.scheme === 'basic'"
                    v-model="authPassword"
                    :disabled="running || oauthBusy"
                    class="field-control mt-2"
                    type="password"
                    :placeholder="t('specification.openapiPassword')"
                    name="openapi-auth-password"
                    :aria-label="t('specification.openapiPassword')"
                    autocomplete="current-password"
                  />
                  <input
                    v-if="selectedAuthScheme !== 'specification' && selectedAuthScheme !== 'none' && !(authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.type === 'http' && authSchemes.find((item) => item.name === selectedAuthScheme)?.scheme.scheme === 'basic')"
                    v-model="authValue"
                    :disabled="running || oauthBusy"
                    class="field-control mt-2"
                    type="password"
                    :placeholder="t('specification.openapiAuthValue')"
                    name="openapi-auth-value"
                    :aria-label="t('specification.openapiAuthValue')"
                    autocomplete="off"
                  />
                  <div v-if="selectedOAuthScheme" class="oauth-flow-controls">
                    <p class="oauth-flow-note">{{ t("specification.openapiOAuthPkceDescription") }}</p>
                    <label>
                      <span>{{ t("specification.openapiOAuthClientId") }}</span>
                      <input v-model="oauthClientId" :disabled="running || oauthBusy" class="field-control" name="openapi-oauth-client-id" autocomplete="off" :placeholder="t('specification.openapiOAuthClientIdPlaceholder')" />
                    </label>
                    <label>
                      <span>{{ t("specification.openapiOAuthScopes") }}</span>
                      <input v-model="oauthScopes" :disabled="running || oauthBusy" class="field-control" name="openapi-oauth-scopes" autocomplete="off" :placeholder="t('specification.openapiOAuthScopesPlaceholder')" />
                    </label>
                    <div class="oauth-redirect-uri">
                      <span>{{ t("specification.openapiOAuthRedirectUri") }}</span>
                      <code>{{ oauthRedirectUri() }}</code>
                    </div>
                    <div class="oauth-flow-actions">
                      <Button size="sm" :disabled="oauthBusy || !oauthClientId.trim()" @click="authorizeOAuth">
                        <LoaderCircle v-if="oauthBusy" class="h-3.5 w-3.5 animate-spin" />
                        <ExternalLink v-else class="h-3.5 w-3.5" />
                        {{ selectedOAuthToken ? t("specification.openapiOAuthReauthorize") : t("specification.openapiOAuthAuthorize") }}
                      </Button>
                      <Button v-if="selectedOAuthToken?.refreshToken" size="sm" variant="outline" :disabled="oauthBusy" @click="refreshOAuthToken"><RotateCcw class="h-3.5 w-3.5" />{{ t("specification.openapiOAuthRefresh") }}</Button>
                      <Button v-if="selectedOAuthToken" size="sm" variant="ghost" :disabled="oauthBusy" @click="clearOAuthToken"><X class="h-3.5 w-3.5" />{{ t("specification.openapiOAuthClear") }}</Button>
                      <span v-if="selectedOAuthToken" class="oauth-token-status" :class="{ expired: selectedOAuthTokenExpired }">{{ selectedOAuthTokenExpired ? t("specification.openapiOAuthTokenExpired") : t("specification.openapiOAuthTokenReady") }}</span>
                    </div>
                    <p v-if="oauthStatus" class="oauth-flow-success" role="status">{{ oauthStatus }}</p>
                    <p v-if="oauthError" class="oauth-flow-error" role="alert">{{ oauthError }}</p>
                  </div>
                </div>
                <OpenApiDebugBody :available-media-types="requestBodyMedia.map(([mediaType]) => mediaType)" v-show="activeDebugSection === 'body'" v-model="requestMediaType" :disabled="running">
                  <template #tools>
                    <AppSelect v-if="requestBodyExamples.length && !isFormMediaType(requestMediaType)" v-model="requestExampleName" size="compact" class="debug-body-example" name="openapi-request-example" :aria-label="t('specification.openapiSelectExample')" @change="applyRequestExample">
                      <option v-for="example in requestBodyExamples" :key="example.name" :value="example.name">{{ example.label }}</option>
                    </AppSelect>
                    <button v-if="isFormMediaType(requestMediaType)" type="button" class="debug-bulk-trigger" @click="formBulkOpen = true">{{ t("specification.openapiBulkEdit") }}</button>
                    <button v-if="requestMediaTypeIsJson" type="button" class="debug-add-button" @click="formatRequestBody"><FileJson class="size-3.5" />{{ t("specification.openapiFormatJson") }}</button>
                  </template>
                  <OpenApiDebugFormTable :document="document" v-if="isFormMediaType(requestMediaType) && !binaryRequestBody" v-model="formRows" :multipart="requestMediaType === 'multipart/form-data'" />
                  <div v-else-if="binaryRequestBody" class="request-file-control mt-2">
                    <input class="field-control" type="file" :aria-label="t('specification.openapiRequestFile')" name="openapi-request-file" @change="selectRequestFile" />
                    <small>{{ requestFile?.name ?? t("specification.openapiNoRequestFile") }}</small>
                  </div>
                  <OpenApiPayloadEditor v-else v-model="requestBodyText" class="mt-2" :media-type="requestMediaType" :label="t('specification.openapiRequestBody')" name="openapi-request-body" compact />
                  <p v-if="requestMediaTypeIsJson && requestBodyText.trim() && !requestBodyJsonValid" class="mt-2 text-xs text-destructive" role="alert">{{ t("specification.openapiInvalidJsonBody") }}</p>
                </OpenApiDebugBody>
                <OpenApiBulkEditDialog v-model:open="formBulkOpen" :rows="formRows.map((row) => ({ ...row, type: row.isFile ? 'file' : row.type || 'string' }))" @apply="formRows = reconcileFormBulkRows(formRows, $event, requestMediaType === 'multipart/form-data')" />
                <div v-show="activeDebugSection === 'before'" class="debug-field-group openapi-script-group">
                  <OpenApiScriptSequence
                    v-model="preRequestScripts"
                    phase="before"
                    :label="t('specification.openapiPreRequestScript')"
                    :add-label="t('specification.openapiAddPreRequestScript')"
                    :help-label="t('specification.openapiScriptHelp')"
                    :move-up-label="t('specification.openapiMoveScriptUp')"
                    :move-down-label="t('specification.openapiMoveScriptDown')"
                    :remove-label="t('common.delete')"
                    :snippets-label="t('specification.openapiScriptSnippets')"
                    :snippets="scriptSnippets"
                    @help="openScriptHelp"
                  />
                </div>
                <div v-show="activeDebugSection === 'after'" class="debug-field-group openapi-script-group">
                  <button type="button" class="debug-add-button" @click="addVisualizationDemo">{{ t("specification.openapiVisualizationDemo") }}</button>
                  <OpenApiScriptSequence
                    v-model="postResponseScripts"
                    phase="after"
                    :label="t('specification.openapiPostResponseScript')"
                    :add-label="t('specification.openapiAddPostResponseScript')"
                    :help-label="t('specification.openapiScriptHelp')"
                    :move-up-label="t('specification.openapiMoveScriptUp')"
                    :move-down-label="t('specification.openapiMoveScriptDown')"
                    :remove-label="t('common.delete')"
                    :snippets-label="t('specification.openapiScriptSnippets')"
                    :snippets="postResponseSnippets"
                    @help="openScriptHelp"
                  />
                </div>
                <OpenApiDebugSettings v-show="activeDebugSection === 'settings'" v-model:timeout="requestTimeoutSeconds" v-model:redirects="followRedirects" v-model:validate-response="autoValidateResponse" />
              </div>
            </section>
            <div v-if="requestError" class="openapi-request-error rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive" role="alert">
              <span>{{ requestError }}</span>
              <button v-if="requestErrorPhase" type="button" class="text-xs underline" @click="activeDebugSection = requestErrorPhase">{{ t("specification.openapiLocateScript") }}</button>
              <button type="button" class="icon-button" :aria-label="t('common.close')" :title="t('common.close')" @click="requestError = ''"><X class="size-3.5" /></button>
            </div>
          </template>
          <template #response>
            <OpenApiDebugResponsePanel
              v-model:validate="autoValidateResponse"
              v-model:status="selectedResponseDefinitionValue"
              v-model:tab="responseTab"
              :tabs="responseViewTabs"
              :status-options="responseDefinitionOptions"
              :running="running"
              :has-response="Boolean(responsePreview)"
              :failed="Boolean(requestError)"
              :metrics="responsePreview?.metrics"
              :status-code="responsePreview?.status"
              :status-text="responsePreview?.statusText"
              :validation="responseSchema !== null ? contractResult : null"
            >
              <div v-if="responsePreview || requestError" class="request-response">
                <div role="tabpanel" :aria-labelledby="`openapi-response-tab-${responseTab}`" tabindex="0">
                  <template v-if="responseTab === 'body'">
                    <template v-if="responsePreview">
                      <p v-if="responsePreview.truncated" class="response-browser-note" role="status">{{ t("specification.openapiResponseTruncated") }}</p>
                      <OpenApiResponseViewer
                        :bytes="responsePreview.rawBytes"
                        :body="responsePreview.body"
                        :content-type="responsePreview.contentType"
                        :image="responsePreview.imageDataUrl"
                        :binary="responsePreview.fileDataUrl"
                        :visualize="responsePreview.visualize"
                        @copy="copyResponse"
                        @download="downloadResponse"
                        @visualization-demo="addVisualizationDemo"
                      />
                      <span class="sr-only" role="status">{{ responseCopyStatus }}</span>
                    </template>
                    <div v-else class="response-failed-view">
                      <strong>{{ t("specification.openapiResponseDetails.failed") }}</strong>
                      <p>{{ requestError }}</p>
                      <button v-if="requestErrorPhase" type="button" class="debug-add-button" @click="activeDebugSection = requestErrorPhase">{{ t("specification.openapiLocateScript") }}</button>
                    </div>
                  </template>
                  <OpenApiResponseCookies v-else-if="responseTab === 'cookies'" :headers="responsePreview?.headers ?? []" :url="responsePreview?.agent?.url || executedRequest?.url || ''" :browser="!responsePreview?.agent" />
                  <template v-else-if="responseTab === 'headers'"
                    ><OpenApiResponseHeaders :headers="responsePreview?.headers ?? []" />
                    <p v-if="!responsePreview?.agent" class="response-browser-note">{{ t("specification.openapiResponseDetails.headerRestricted") }}</p></template
                  >
                  <OpenApiResponseConsole v-else-if="responseTab === 'console'" :logs="responsePreview?.scriptLogs ?? scriptLogs" :tests="responsePreview?.scriptTests ?? scriptTests" :error="responsePreview?.scriptError || scriptError || requestError" />
                  <template v-else-if="responseTab === 'request'"
                    ><OpenApiExecutedRequest v-if="executedRequest" :request="executedRequest" />
                    <p v-else class="response-view-empty">{{ t("specification.openapiResponseDetails.noRequest") }}</p></template
                  >
                </div>
              </div>
            </OpenApiDebugResponsePanel>
          </template>
        </OpenApiDebugSplitPane>
        <OpenApiRequestProxyControl v-if="!publicTryIt && requestAgent" :environment-mode="environmentProxyMode" />
      </section>
      <section v-if="designViewVisited && props.serviceId" v-show="activeOperationView === 'design'" id="openapi-operation-design" class="openapi-design-view" role="tabpanel" aria-labelledby="openapi-operation-design-tab" tabindex="0">
        <OpenApiDesignPanel
          :content="props.content"
          :focus-operation-key="props.focusOperationKey"
          :draft-scope="props.designDraftScope"
          :editable="props.editable"
          :save-operation="props.saveOperation"
          :captured-example="capturedResponseExample"
          @captured="capturedResponseExample = null"
          @debug="selectOperationView('debug')"
        />
      </section>
      <OpenApiResponseExampleDialog
        v-if="showResponseExample && responseExampleTarget && responsePreview"
        :target="responseExampleTarget"
        :initial-body="responsePreview.body"
        :names="responseExampleTarget.names"
        :capture="responseExampleTarget"
        @close="closeResponseExample"
        @apply="stageResponseExample"
      />
    </template>
    <div v-else class="border-l-2 border-amber-500 py-2 pl-4 text-sm text-muted-foreground" role="status">{{ t("specification.openapiOperationRemoved") }}</div>
  </article>

  <article v-else-if="document" class="min-w-0" :aria-label="t('specification.openapiPreview')">
    <header class="flex flex-wrap items-end justify-between gap-4 border-b border-border pb-5">
      <div class="min-w-0">
        <span class="eyebrow">OpenAPI {{ document.openapi }}</span>
        <h3 class="mt-1 break-words text-xl font-semibold">{{ info?.title || "OpenAPI" }}</h3>
        <p v-if="info?.description" class="mt-2 max-w-3xl whitespace-pre-wrap text-sm leading-6 text-muted-foreground">{{ info.description }}</p>
      </div>
      <div class="flex shrink-0 items-baseline gap-2 text-sm">
        <strong class="font-mono tabular-nums">{{ operations.length }}</strong
        ><span class="text-muted-foreground">{{ t("specification.openapiOperations") }}</span
        ><span v-if="info?.version" class="ml-2 border-l border-border pl-3 font-mono text-xs text-muted-foreground">{{ info.version }}</span>
      </div>
    </header>
    <section v-if="servers.length" class="border-b border-border py-4">
      <h4 class="mb-2 text-xs font-semibold uppercase text-muted-foreground">{{ t("specification.openapiServers") }}</h4>
      <div class="flex flex-wrap gap-x-5 gap-y-2">
        <a
          v-for="server in servers.filter((item) => safeExternalUrl(String(item.url)))"
          :key="String(server.url)"
          class="inline-flex max-w-full items-center gap-1.5 font-mono text-xs text-primary hover:underline"
          :href="safeExternalUrl(String(server.url)) ?? undefined"
          target="_blank"
          rel="noreferrer"
          ><ExternalLink class="h-3 w-3 shrink-0" aria-hidden="true" /><span class="truncate">{{ server.url }}</span></a
        >
        <span v-if="servers.some((server) => !safeExternalUrl(String(server.url)))" class="text-xs text-destructive">{{ t("specification.openapiUnsafeServer") }}</span>
      </div>
    </section>
    <section class="py-4">
      <div class="mb-2 flex items-center justify-between">
        <h4 class="text-sm font-semibold">{{ t("specification.openapiOperations") }}</h4>
        <span class="text-xs text-muted-foreground">{{ t("specification.openapiMethod") }} · {{ t("specification.openapiPath") }}</span>
      </div>
      <div v-if="operations.length" class="divide-y divide-border border-y border-border">
        <div v-for="operation in operations" :key="operation.identityKey" class="grid min-w-0 grid-cols-[4.5rem_minmax(0,1fr)] gap-3 py-3">
          <span class="h-fit rounded px-2 py-1 text-center font-mono text-[10px] font-bold" :class="openApiMethodClass(operation.method.toUpperCase())">{{ operation.method.toUpperCase() }}</span>
          <div class="min-w-0">
            <code class="block break-all text-sm">{{ operation.path }}</code>
            <p v-if="operation.operation.summary || operation.operation.operationId" class="mt-1 text-xs text-muted-foreground">{{ operation.operation.summary || operation.operation.operationId }}</p>
            <div class="mt-1 flex flex-wrap gap-1.5">
              <span v-for="tag in operation.tags" :key="tag" class="text-[10px] text-muted-foreground">{{ tag }}</span>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="flex items-center gap-2 py-8 text-sm text-muted-foreground"><Braces class="h-4 w-4" aria-hidden="true" />{{ t("specification.openapiNoOperations") }}</div>
    </section>
  </article>
  <pre v-else class="max-h-[32rem] overflow-auto rounded-md border border-border bg-muted/10 p-4 font-mono text-xs leading-relaxed" :aria-label="t('specification.openapiRawFallback')">{{ content }}</pre>
</template>
