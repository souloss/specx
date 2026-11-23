<script setup lang="ts">
import { computed, ref, useId } from "vue";
import OpenApiDocumentMetadata from "./OpenApiDocumentMetadata.vue";
import MarkdownPreview from "../../markdown/MarkdownPreview.vue";
import OpenApiMediaDocument from "./OpenApiMediaDocument.vue";
import OpenApiParameterDocument from "./OpenApiParameterDocument.vue";
import OpenApiResponseDocument from "./OpenApiResponseDocument.vue";
import OpenApiSecurityDocument from "./OpenApiSecurityDocument.vue";
import OpenApiPageActions from "./OpenApiPageActions.vue";
import OpenApiRequestCode from "./OpenApiRequestCode.vue";
import OpenApiResponseStatusTabs from "../shared/OpenApiResponseStatusTabs.vue";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import { isRecord, openApiMethodClass, resolveOpenApiRef, type OpenApiObject, type OpenApiOperationEntry } from "@/lib/specifications/openapi/contract/openapi";
import { operationOpenApiDocument } from "@/lib/specifications/openapi/documentation/openapiDocumentExport";
import { getOperationParameters } from "@/lib/specifications/openapi/contract/openapiParameters";
import { operationMarkdown } from "@/lib/specifications/openapi/documentation/openapiDocumentation";
import { translate as t } from "@/i18n";
import { Zap } from "@lucide/vue";
import { Button } from "@/components/ui/button";
const emit = defineEmits<{ debug: [] }>();

const props = withDefaults(
  defineProps<{
    document: OpenApiObject | null;
    entry: OpenApiOperationEntry;
    specificationTitle?: string;
    specificationVersion?: number;
    showOperationId?: boolean;
    schemaDisplay?: "summary" | "full";
    navigateModel?: (reference: string) => void;
    allowExport?: boolean;
    canDebug?: boolean;
    serviceId?: string;
    showExamples?: boolean;
  }>(),
  {
    showOperationId: true,
    schemaDisplay: "full",
    specificationTitle: "",
    specificationVersion: 0,
    allowExport: true,
    showExamples: false,
  },
);
const parameters = computed(() => getOperationParameters(props.document, props.entry));
const requestBody = computed(() => resolveOpenApiRef(props.document, props.entry.operation.requestBody));
const responses = computed(() =>
  isRecord(props.entry.operation.responses)
    ? Object.entries(props.entry.operation.responses).flatMap(([status, value]): Array<[string, OpenApiObject]> => {
        const response = resolveOpenApiRef(props.document, value);
        return response ? [[status, response]] : [];
      })
    : [],
);
const status = ref("");
const responseMediaType = ref("");
const markdown = computed(() => operationMarkdown(props.document, props.entry));
const definition = computed(() => JSON.stringify(operationOpenApiDocument(props.document, props.entry), null, 2));
const hasSecurity = computed(() => {
  const security = props.entry.operation.security ?? props.document?.security;
  return Array.isArray(security) && security.some((item) => isRecord(item) && Object.keys(item).length > 0);
});
const title = computed(() => {
  for (const value of [props.entry.operation.summary, props.entry.operation.operationId, props.entry.path]) {
    if (typeof value === "string" && value.trim()) return value;
  }
  return props.entry.path;
});
const selectedResponse = computed(() => responses.value.find(([code]) => code === status.value) ?? responses.value[0]);
const exampleId = useId();
const responseExampleTabs = computed(() => responses.value.map(([code], index) => ({ value: code, id: `${exampleId}-response-example-${index}`, label: "" })));
const selectedExampleTab = computed(() => responseExampleTabs.value.find((tab) => tab.value === selectedResponse.value?.[0]));
</script>

<template>
  <div class="openapi-operation-document min-w-0">
    <div class="openapi-document-columns" :class="{ 'openapi-document-columns--with-examples': props.showExamples }">
      <div class="openapi-document-flow min-w-0">
        <header class="openapi-operation-base">
          <div class="openapi-operation-base-heading">
            <div class="min-w-0">
              <div class="openapi-operation-title-row">
                <h2 class="min-w-0 break-words text-xl font-semibold tracking-normal">{{ title }}</h2>
                <span v-if="props.showOperationId && props.entry.operation.operationId" class="openapi-operation-id">
                  <span class="font-medium text-muted-foreground">operationId</span>
                  <code translate="no">{{ props.entry.operation.operationId }}</code>
                </span>
              </div>
            </div>
            <div class="openapi-document-header-actions">
              <OpenApiPageActions :markdown="markdown" :definition="definition" :allow-export="allowExport" /><Button v-if="canDebug" size="sm" @click="emit('debug')"><Zap class="size-3.5" aria-hidden="true" />{{ t("specification.openapiDebugTab") }}</Button>
            </div>
          </div>
          <div class="openapi-operation-route-row">
            <code class="openapi-operation-method" :class="openApiMethodClass(props.entry.method.toUpperCase())" translate="no">{{ props.entry.method.toUpperCase() }}</code>
            <code class="openapi-operation-path" translate="no">{{ props.entry.path }}</code>
          </div>
          <div class="openapi-operation-meta">
            <span v-if="props.specificationTitle">{{ props.specificationTitle }} · {{ t("specification.currentVersion", { version: props.specificationVersion }) }}</span>
            <span v-for="tag in props.entry.tags" :key="tag" class="openapi-operation-tag">{{ tag }}</span>
          </div>
          <OpenApiDocumentMetadata :external-docs="props.entry.operation.externalDocs" />
          <MarkdownPreview v-if="typeof props.entry.operation.description === 'string' && props.entry.operation.description.trim()" :content="props.entry.operation.description" class="text-sm" />
        </header>
        <OpenApiSecurityDocument v-if="hasSecurity" :document="props.document" :entry="props.entry" />
        <section v-if="parameters.length">
          <h3 class="openapi-document-section-title">{{ t("specification.openapiParameters") }}</h3>
          <OpenApiParameterDocument :allow-export="props.allowExport" :parameters="parameters" :document="props.document" :navigate-model="props.navigateModel" />
        </section>
        <section v-if="requestBody">
          <div class="openapi-document-section-heading">
            <h3 class="openapi-document-section-title">{{ t("specification.openapiRequestBody") }}</h3>
            <span v-if="requestBody.required" class="schema-required">{{ t("specification.openapiRequired") }}</span>
          </div>
          <MarkdownPreview v-if="typeof requestBody.description === 'string'" :content="requestBody.description" class="mb-3 text-sm" />
          <OpenApiMediaDocument
            v-if="isRecord(requestBody.content)"
            class="rounded-lg border border-border bg-card p-4"
            :allow-export="props.allowExport"
            :content="requestBody.content"
            :document="props.document"
            :schema-display="props.schemaDisplay"
            :navigate-model="props.navigateModel"
            :show-examples="!props.showExamples"
          />
        </section>
        <OpenApiResponseDocument :allow-export="props.allowExport" v-model:status="status" v-model:media-type="responseMediaType" :responses="responses" :document="props.document" :schema-display="props.schemaDisplay" :navigate-model="props.navigateModel" :show-examples="!props.showExamples" />
      </div>
      <aside v-if="props.showExamples" class="openapi-document-examples" :aria-label="t('openapiPublication.requestExample')">
        <section class="openapi-document-example-card openapi-document-request-example">
          <OpenApiRequestCode :allow-export="props.allowExport" :document="props.document" :entry="props.entry" />
        </section>
        <section v-if="selectedResponse" class="openapi-document-example-card">
          <h4>{{ t("openapiPublication.responsePreview") }}</h4>
          <OpenApiResponseStatusTabs v-if="responseExampleTabs.length > 1" :model-value="selectedResponse[0]" :tabs="responseExampleTabs" :ariaLabel="t('openapiPublication.responseStatus')" @update:model-value="status = $event" />
          <div class="openapi-document-example-content" :role="responseExampleTabs.length > 1 ? 'tabpanel' : undefined" :aria-labelledby="responseExampleTabs.length > 1 ? selectedExampleTab?.id : undefined">
            <OpenApiMediaDocument
              v-if="isRecord(selectedResponse[1].content)"
              v-model:media-type="responseMediaType"
              :content="selectedResponse[1].content"
              :document="props.document"
              :schema-display="props.schemaDisplay"
              :navigate-model="props.navigateModel"
              :allow-export="props.allowExport"
              :show-schema="false"
              layout="stacked"
            />
            <OpenApiSchemaPreview v-else-if="selectedResponse[1].schema !== undefined" :schema="selectedResponse[1].schema" :document="props.document" :mode="props.schemaDisplay" :navigate-model="props.navigateModel" />
            <p v-else class="text-xs text-muted-foreground">{{ t("openapiPublication.noResponseBody") }}</p>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>

<style>
.openapi-operation-document {
  container: openapi-document / inline-size;
}
.openapi-operation-base {
  display: grid;
  min-width: 0;
  gap: 10px;
  padding-block-start: 20px;
}
.openapi-operation-base-heading {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
}
.openapi-operation-title-row {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 10px 14px;
}
.openapi-operation-id {
  display: inline-flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 5px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.openapi-operation-id code {
  overflow-wrap: anywhere;
  color: var(--foreground);
  font-family: var(--font-mono);
}
.openapi-operation-route-row {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}
.openapi-operation-method {
  display: inline-flex;
  min-height: 24px;
  align-items: center;
  border-radius: 5px;
  padding: 0 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
}
.openapi-operation-path {
  min-width: 0;
  overflow-wrap: anywhere;
  color: var(--foreground);
  font-family: var(--font-mono);
  font-size: 13px;
}
.openapi-operation-meta {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.openapi-operation-tag {
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 2px 6px;
  color: var(--foreground);
  background: var(--surface-muted);
}
.openapi-document-flow {
  display: grid;
  gap: 24px;
  padding-bottom: 32px;
}
.openapi-document-columns {
  display: grid;
  min-width: 0;
  grid-template-columns: minmax(0, 1fr);
  gap: 28px;
  align-items: start;
}
.openapi-document-examples {
  display: grid;
  min-width: 0;
  gap: 16px;
  position: sticky;
  inset-block-start: 16px;
  max-block-size: calc(100dvh - 32px);
  overflow-y: auto;
  scrollbar-gutter: stable;
}
.openapi-document-example-card {
  min-width: 0;
  overflow: clip;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-muted);
}
.openapi-document-example-card > h4 {
  margin: 0;
  padding: 11px 13px;
  border-block-end: 1px solid var(--border);
  color: var(--foreground);
  font-size: 13px;
  font-weight: 650;
}
.openapi-document-example-content {
  min-inline-size: 0;
  padding: 12px;
}
.openapi-document-example-card > .openapi-request-code {
  margin: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
}
.openapi-document-example-card .openapi-request-code {
  margin: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
}
.openapi-document-examples .openapi-code-sample {
  max-block-size: 24rem;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  word-break: break-word;
}
.openapi-document-example-card .openapi-request-code > :first-child {
  border-block-start: 0;
}
.openapi-document-example-card .openapi-media-panes {
  grid-template-columns: minmax(0, 1fr);
}
.openapi-document-example-card .openapi-example-card {
  border: 0;
  border-radius: 0;
}
.openapi-document-example-card .openapi-operation-tabs {
  padding-inline: 13px;
  background: transparent;
}
@container openapi-document (min-width: 52rem) {
  .openapi-document-columns--with-examples {
    grid-template-columns: minmax(0, 1fr) minmax(260px, 340px);
  }
}
@container openapi-document (width < 52rem) {
  .openapi-document-examples {
    position: static;
    max-block-size: none;
    overflow: visible;
  }
}
.openapi-document-header-actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: 10px;
}
.openapi-parameter-document-heading {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}
.openapi-parameter-document-name {
  border-radius: 4px;
  padding: 2px 6px;
  color: #3297ff;
  background: color-mix(in srgb, #3297ff 5%, transparent);
  font-size: 12px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.openapi-parameter-document-description {
  min-width: 0;
  overflow-wrap: anywhere;
  color: var(--muted-foreground);
  font-size: 12px;
}
.openapi-parameter-document-heading .schema-required {
  margin-inline-start: auto;
}
.openapi-parameter-inline-example {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  font-size: 12px;
  color: var(--muted-foreground);
}
.openapi-parameter-inline-example code {
  min-inline-size: 0;
  overflow-wrap: anywhere;
  border-radius: 4px;
  padding: 1px 6px;
  background: var(--surface-muted);
  font-size: 11px;
}
.openapi-document-section-title {
  margin: 0 0 12px;
  font-size: 16px;
  font-weight: 650;
}
.openapi-document-section-heading {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 12px;
}
.openapi-document-section-heading .openapi-document-section-title {
  margin-bottom: 0;
}
.openapi-operation-document .markdown-preview {
  min-height: 0;
  max-height: none;
  border: 0;
  padding: 0;
  background: transparent;
  box-shadow: none;
}
.openapi-operation-document .markdown-preview > .markdown-body {
  padding: 0;
}
.openapi-operation-document .schema-preview {
  font-size: 12px;
}
.openapi-operation-document .schema-reference {
  display: none;
}
.openapi-mock-heading {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 16px;
  font-weight: 600;
}
.openapi-mock-document .openapi-operation-tabs {
  gap: 16px;
}
.openapi-mock-document .openapi-operation-tab {
  min-height: 40px;
}
.openapi-mock-table-scroll {
  min-width: 0;
  overflow-x: auto;
}
.openapi-mock-table {
  width: 100%;
  min-width: 640px;
  border-collapse: collapse;
  table-layout: fixed;
  font-size: 14px;
}
.openapi-mock-table th,
.openapi-mock-table td {
  height: 33px;
  padding: 2px 12px;
  border-bottom: 1px solid var(--border);
  text-align: left;
  font-weight: 400;
}
.openapi-mock-table th {
  color: var(--muted-foreground);
}
.openapi-mock-table th:nth-child(1) {
  width: 20%;
}
.openapi-mock-table th:nth-child(2) {
  width: 180px;
}
.openapi-mock-table th:last-child {
  width: 110px;
}
.openapi-mock-table td code {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}
.openapi-mock-table td button {
  min-height: 24px;
  height: 26px;
  padding-block: 0;
  font-size: 12px;
}
.openapi-mock-enable {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  font-size: 12px;
  color: var(--muted-foreground);
}
.openapi-operation-document .schema-fields {
  border: 0;
  border-radius: 0;
  margin-top: 8px;
}
.openapi-operation-document .schema-fields-heading {
  background: transparent;
  padding: 0;
}
.openapi-operation-document .schema-field {
  border: 0;
  padding: 10px 8px;
}
.openapi-operation-document .schema-field-type {
  margin-left: 0;
  font-size: 12px;
}
.openapi-operation-document .schema-field-main > .schema-required {
  order: 2;
  margin-inline-start: auto;
  color: #ef8b25;
}
.openapi-operation-document .schema-fields-body {
  border: 0;
}
.openapi-operation-document .schema-preview-nested > .schema-fields {
  border-left: 1px solid var(--border);
}
.openapi-operation-document .openapi-media-document--split .openapi-example-card {
  border: 0;
  border-radius: 0;
  background: var(--background);
}
.openapi-operation-document .openapi-media-document--split .openapi-example-card > div:first-child {
  background: transparent;
  border: 0;
  padding-top: 0;
}
.openapi-operation-document .openapi-media-panes--split {
  grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
}
@container openapi-response (max-width: 46rem) {
  .openapi-operation-document .openapi-media-panes--split {
    grid-template-columns: minmax(0, 1fr);
  }
}
@container openapi-document (max-width: 34rem) {
  .openapi-operation-base-heading {
    flex-direction: column;
  }
}
</style>
