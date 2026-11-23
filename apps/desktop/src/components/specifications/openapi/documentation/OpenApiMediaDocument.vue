<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Check, Clipboard, Download } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { formatOpenApiExample, generateOpenApiExample, getOpenApiMediaExamples, isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";
import { translate as t } from "@/i18n";
import type { OpenApiRequestExampleSelection } from "@/lib/specifications/openapi/documentation/openapiDocumentation";

const props = withDefaults(defineProps<{ document: OpenApiObject | null; content: OpenApiObject; schemaDisplay?: "summary" | "full"; navigateModel?: (reference: string) => void; showSchema?: boolean; showExamples?: boolean; label?: string; allowExport?: boolean; layout?: "stacked" | "split" }>(), {
  schemaDisplay: "full",
  showSchema: true,
  showExamples: true,
  label: "",
  allowExport: true,
  layout: "stacked",
});
const emit = defineEmits<{ selectExample: [selection: OpenApiRequestExampleSelection] }>();
const mediaType = defineModel<string>("mediaType", { default: "" });
const selection = ref<Record<string, string>>({});
const entries = computed(() => Object.entries(props.content).filter((entry): entry is [string, OpenApiObject] => isRecord(entry[1])));
const selected = computed(() => entries.value.find(([type]) => type === mediaType.value) ?? entries.value[0]);
const examples = computed(() => {
  if (!selected.value) return [];
  const explicit = getOpenApiMediaExamples(props.document, selected.value[1]);
  if (explicit.length) return explicit;
  if (selected.value[1].schema === undefined) return [];
  return [{ name: "generated", label: t("specification.openapiGeneratedExample", { mediaType: selected.value[0] }), description: "", value: generateOpenApiExample(props.document, selected.value[1].schema) }];
});
const example = computed(() => examples.value.find((item) => item.name === selection.value[selected.value?.[0] ?? ""]) ?? examples.value[0]);
const split = computed(() => props.layout === "split" && props.showSchema && selected.value?.[1].schema !== undefined && props.showExamples && Boolean(example.value));
watch(
  () => ({ mediaType: selected.value?.[0] ?? "", exampleName: example.value?.name ?? "" }),
  (value) => emit("selectExample", value),
  { immediate: true },
);
const formatted = computed(() => (example.value && selected.value ? formatOpenApiExample(example.value.value, selected.value[0]) : ""));
const { copy, copied, failed } = useClipboardFeedback();
function chooseExample(value: string) {
  if (selected.value) selection.value[selected.value[0]] = value;
}
function download() {
  if (!selected.value || !example.value) return;
  const type = selected.value[0];
  const url = URL.createObjectURL(new Blob([formatted.value], { type: type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `example.${type.includes("json") ? "json" : type.includes("xml") ? "xml" : "txt"}`;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
</script>

<template>
  <div v-if="selected" class="openapi-media-document min-w-0" :class="{ 'openapi-media-document--split': split }">
    <div class="mb-3 flex min-w-0 flex-wrap items-center gap-2">
      <span v-if="label" class="text-xs font-semibold text-muted-foreground">{{ label }}</span>
      <AppSelect v-if="entries.length > 1" :value="selected[0]" size="compact" :aria-label="t('specification.openapiMediaType')" class="max-w-full text-xs" @update:model-value="mediaType = $event">
        <option v-for="[type] in entries" :key="type" :value="type">{{ type }}</option>
      </AppSelect>
      <code v-else class="min-w-0 wrap-anywhere rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground" translate="no">{{ selected[0] }}</code>
    </div>
    <div class="openapi-media-panes" :class="{ 'openapi-media-panes--split': split }">
      <div v-if="showSchema && selected[1].schema !== undefined" class="openapi-media-schema-pane">
        <OpenApiSchemaPreview :schema="selected[1].schema" :document="document" :mode="schemaDisplay" :navigate-model="navigateModel" />
      </div>
      <section v-if="showExamples && example" class="openapi-example-card overflow-hidden rounded-lg border border-border bg-card">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2 border-b border-border bg-muted/30 px-3 py-2">
          <AppSelect :value="example.name" size="compact" :aria-label="t('specification.openapiSelectExample')" class="min-w-0 max-w-full text-xs" @update:model-value="chooseExample">
            <option v-for="item in examples" :key="item.name" :value="item.name">{{ item.label }}</option>
          </AppSelect>
          <div class="flex shrink-0 items-center gap-1">
            <button type="button" class="icon-button" :aria-label="t('specification.openapiCopyExample')" @click="copy(formatted)"><Check v-if="copied" class="size-3.5" aria-hidden="true" /><Clipboard v-else class="size-3.5" aria-hidden="true" /></button>
            <button v-if="allowExport" type="button" class="icon-button" :aria-label="t('openapiPublication.downloadExample')" @click="download"><Download class="size-3.5" aria-hidden="true" /></button>
          </div>
        </div>
        <p v-if="example.description" class="px-3 pt-3 text-xs leading-5 text-muted-foreground">{{ example.description }}</p>
        <OpenApiCodeSample :text="formatted" :language="selected[0]" />
        <p v-if="copied || failed" role="status" class="border-t border-border px-3 py-2 text-xs text-muted-foreground">{{ t(failed ? "specification.openapiCopyFailed" : "specification.openapiExampleCopied") }}</p>
      </section>
    </div>
  </div>
</template>
