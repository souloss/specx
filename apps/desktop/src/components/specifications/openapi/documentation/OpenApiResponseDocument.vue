<script setup lang="ts">
import { computed, useId } from "vue";
import OpenApiResponseStatusTabs from "../shared/OpenApiResponseStatusTabs.vue";
import MarkdownPreview from "../../markdown/MarkdownPreview.vue";
import OpenApiMediaDocument from "./OpenApiMediaDocument.vue";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import { isRecord, resolveOpenApiRef, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";
const props = withDefaults(defineProps<{ allowExport?: boolean; responses: Array<[string, OpenApiObject]>; document: OpenApiObject | null; schemaDisplay: "summary" | "full"; navigateModel?: (reference: string) => void; showExamples?: boolean }>(), { showExamples: true });
const status = defineModel<string>("status", { default: "" });
const mediaType = defineModel<string>("mediaType", { default: "" });
const selected = computed(() => props.responses.find(([key]) => key === status.value) ?? props.responses[0]);
const tabId = useId();
const responseTabs = computed(() => props.responses.map(([code, response], index) => ({ value: code, id: `${tabId}-response-${index}`, label: typeof response.description === "string" ? response.description : "" })));
const selectedTabId = computed(() => responseTabs.value.find((tab) => tab.value === selected.value?.[0])?.id);
const headers = computed(() => (isRecord(selected.value?.[1].headers) ? Object.entries(selected.value[1].headers).map(([name, value]) => [name, resolveOpenApiRef(props.document, value)] as const) : []));
</script>
<template>
  <section v-if="selected" class="openapi-response-document min-w-0" :aria-label="t('openapiPublication.responsePreview')">
    <h3 class="openapi-document-section-title">{{ t("specification.openapiResponses") }}</h3>
    <div class="openapi-response-panel">
      <OpenApiResponseStatusTabs :model-value="selected[0]" :tabs="responseTabs" :ariaLabel="t('openapiPublication.responseStatus')" @update:model-value="status = $event" />
      <div class="openapi-response-panel-body" role="tabpanel" tabindex="0" :aria-labelledby="selectedTabId">
        <details v-if="headers.length" open class="openapi-response-headers">
          <summary>{{ t("specification.openapiResponseHeaders") }} · {{ headers.length }}</summary>
          <dl class="divide-y divide-border px-3">
            <div v-for="[name, header] in headers" :key="name" class="min-w-0 py-3">
              <dt class="break-all font-mono text-xs font-semibold" translate="no">{{ name }}</dt>
              <dd class="mt-2 space-y-2">
                <MarkdownPreview v-if="typeof header?.description === 'string'" :content="header.description" class="text-xs" /><OpenApiSchemaPreview v-if="header?.schema !== undefined" :schema="header.schema" :document="document" :navigate-model="navigateModel" /><OpenApiMediaDocument
                  :allow-export="allowExport"
                  v-if="isRecord(header?.content)"
                  :content="header.content"
                  :document="document"
                  :navigate-model="navigateModel"
                />
              </dd>
            </div>
          </dl>
        </details>
        <OpenApiMediaDocument
          :allow-export="allowExport"
          :show-examples="props.showExamples"
          v-if="isRecord(selected[1].content)"
          :key="selected[0]"
          v-model:media-type="mediaType"
          :content="selected[1].content"
          :document="document"
          :schema-display="schemaDisplay"
          :navigate-model="navigateModel"
          :layout="'split'"
          :label="t('specification.openapiResponseBody')"
        />
        <OpenApiSchemaPreview v-else-if="selected[1].schema !== undefined" :schema="selected[1].schema" :document="document" :mode="schemaDisplay" :navigate-model="navigateModel" />
        <p v-else class="text-xs text-muted-foreground">{{ t("openapiPublication.noResponseBody") }}</p>
      </div>
    </div>
  </section>
</template>
