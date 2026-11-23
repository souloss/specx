<script setup lang="ts">
import { computed, ref } from "vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import OpenApiPageActions from "../documentation/OpenApiPageActions.vue";
import { modelMarkdown } from "@/lib/specifications/openapi/documentation/openapiDocumentExport";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { formatOpenApiExample, generateOpenApiExample, mergeOpenApiSchema, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { getOpenApiModelDependencies, getOpenApiModelUsages, type OpenApiModel, type OpenApiModelUsage } from "@/lib/specifications/openapi/contract/openapiModels";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";
import { translate as t } from "@/i18n";

const props = defineProps<{ document: OpenApiObject | null; model: OpenApiModel; allowExport?: boolean }>();
const emit = defineEmits<{ navigateModel: [reference: string]; navigateOperation: [key: string] }>();
const dependencies = computed(() => (props.document ? getOpenApiModelDependencies(props.document, props.model.reference) : []));
const usages = computed(() => (props.document ? getOpenApiModelUsages(props.document, props.model.reference) : []));
const selection = ref(0);
const examples = computed(() => {
  const schema = mergeOpenApiSchema(props.document, props.model.schema);
  const values = Array.isArray(schema?.examples) ? schema.examples : schema?.example !== undefined ? [schema.example] : [];
  return values.length ? values.map((value, index) => ({ label: `${t("openapiPublication.example")} ${index + 1}`, value })) : [{ label: t("openapiPublication.generatedExample"), value: generateOpenApiExample(props.document, props.model.schema) }];
});
const example = computed(() => formatOpenApiExample((examples.value[selection.value] ?? examples.value[0])?.value, "application/json"));
const { copy, copied, failed } = useClipboardFeedback();
function navigateModel(reference: string) {
  emit("navigateModel", reference);
}
function navigateUsage(usage: OpenApiModelUsage) {
  if (usage.kind === "model") navigateModel(usage.key);
  else emit("navigateOperation", usage.key);
}
</script>

<template>
  <article class="min-w-0 space-y-5">
    <header class="flex flex-wrap items-start justify-between gap-3 border-b border-border pb-4">
      <div>
        <p class="eyebrow">{{ t("openapiPublication.model") }}</p>
        <h2 tabindex="-1" class="mt-1 break-all font-mono text-xl font-semibold">{{ model.name }}</h2>
      </div>
      <OpenApiPageActions :markdown="modelMarkdown(model, document)" :allow-export="allowExport" />
    </header>
    <section class="rounded-lg border border-border bg-card p-4">
      <OpenApiSchemaPreview :schema="{ $ref: model.reference }" :document="document" mode="full" :navigate-model="navigateModel" />
    </section>
    <section class="overflow-hidden rounded-lg border border-border bg-card">
      <div class="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
        <AppSelect v-if="examples.length > 1" v-model.number="selection" size="compact" :aria-label="t('specification.openapiSelectExample')"
          ><option v-for="(item, index) in examples" :key="index" :value="index">{{ item.label }}</option></AppSelect
        >
        <h3 v-else class="text-sm font-semibold">{{ examples[0]?.label }}</h3>
        <button type="button" class="rounded px-2 py-1 text-xs text-primary hover:bg-accent" @click="copy(example)">{{ t(copied ? "specification.openapiCopied" : "openapiPublication.copyExample") }}</button>
      </div>
      <OpenApiCodeSample :text="example" language="json" />
      <p v-if="failed" role="status" class="px-4 py-2 text-xs text-destructive">{{ t("specification.openapiCopyFailed") }}</p>
    </section>
    <section v-if="dependencies.length || usages.length" class="grid gap-4 sm:grid-cols-2">
      <div v-if="dependencies.length" class="min-w-0 rounded-lg border border-border bg-card p-4">
        <h3 class="text-sm font-semibold">{{ t("openapiPublication.dependsOn") }}</h3>
        <ul class="mt-2 max-h-72 space-y-1.5 overflow-y-auto overscroll-contain">
          <li v-for="item in dependencies" :key="item.reference">
            <button type="button" class="max-w-full break-all text-left text-xs text-primary hover:underline" @click="navigateModel(item.reference)">{{ item.name }}</button>
          </li>
        </ul>
      </div>
      <div v-if="usages.length" class="min-w-0 rounded-lg border border-border bg-card p-4">
        <h3 class="text-sm font-semibold">{{ t("openapiPublication.usedBy") }}</h3>
        <ul class="mt-2 max-h-72 space-y-1.5 overflow-y-auto overscroll-contain">
          <li v-for="usage in usages" :key="`${usage.kind}:${usage.key}`">
            <button type="button" class="max-w-full break-all text-left text-xs text-primary hover:underline" @click="navigateUsage(usage)">{{ usage.title }}</button>
          </li>
        </ul>
      </div>
    </section>
  </article>
</template>
