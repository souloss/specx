<script setup lang="ts">
import { computed } from "vue";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import OpenApiMediaDocument from "./OpenApiMediaDocument.vue";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { formatOpenApiExample, getOpenApiMediaExamples, isRecord, mergeOpenApiSchema, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";
const props = defineProps<{ allowExport?: boolean; parameters: OpenApiObject[]; document: OpenApiObject | null; navigateModel?: (reference: string) => void }>();
const groups = computed(() => {
  const result = new Map<string, OpenApiObject[]>();
  for (const parameter of props.parameters) {
    const location = String(parameter.in ?? "header");
    const entries = result.get(location) ?? [];
    entries.push(parameter);
    result.set(location, entries);
  }
  return [...result];
});
function examples(parameter: OpenApiObject) {
  return getOpenApiMediaExamples(props.document, parameter);
}
function scalarExample(value: unknown) {
  return value === null || ["string", "number", "boolean"].includes(typeof value);
}
function parameterType(parameter: OpenApiObject) {
  const schema = mergeOpenApiSchema(props.document, parameter.schema);
  return Array.isArray(schema?.type) ? schema.type.join(" | ") : typeof schema?.type === "string" ? schema.type : schema?.properties ? "object" : "";
}
function detailsSchema(parameter: OpenApiObject) {
  const schema = mergeOpenApiSchema(props.document, parameter.schema);
  if (!schema) return parameter.schema;
  const details = Object.fromEntries(Object.entries(schema).filter(([key]) => key !== "type" && key !== "description"));
  return Object.keys(details).length ? details : undefined;
}
function serialization(parameter: OpenApiObject): string[] {
  return [typeof parameter.style === "string" ? `style: ${parameter.style}` : "", typeof parameter.explode === "boolean" ? `explode: ${parameter.explode}` : "", parameter.allowReserved === true ? "allowReserved: true" : "", parameter.allowEmptyValue === true ? "allowEmptyValue: true" : ""].filter(
    Boolean,
  );
}
</script>

<template>
  <div class="space-y-4">
    <section v-for="[location, entries] in groups" :key="location" class="openapi-parameter-document overflow-hidden rounded-lg border border-border bg-card">
      <div class="flex items-center gap-2 border-b border-border px-4 py-3 text-xs">
        <h5 class="flex items-center gap-2">
          <template v-if="location === 'path'">{{ t("specification.openapiPathParameters") }}</template>
          <template v-else-if="location === 'query'">{{ t("specification.openapiQueryParameters") }}</template>
          <template v-else
            ><code class="capitalize" translate="no">{{ location }}</code
            ><span>{{ t("specification.openapiParameters") }}</span></template
          >
        </h5>
      </div>
      <dl class="divide-y divide-border/70">
        <div v-for="parameter in entries" :key="String(parameter.name)" class="min-w-0 px-4 py-3.5">
          <dt class="openapi-parameter-document-heading">
            <code class="openapi-parameter-document-name" translate="no">{{ parameter.name }}</code>
            <span class="text-xs text-muted-foreground">{{ parameterType(parameter) }}</span>
            <span v-if="typeof parameter.description === 'string'" class="openapi-parameter-document-description">{{ parameter.description }}</span>
            <span v-if="parameter.required || location === 'path'" class="schema-required">{{ t("specification.openapiRequiredMark") }}</span>
            <span v-else class="text-[11px] text-muted-foreground">{{ t("openapiPublication.optional") }}</span>
          </dt>
          <dd class="mt-2 min-w-0 space-y-2">
            <OpenApiSchemaPreview v-if="detailsSchema(parameter) !== undefined" :schema="detailsSchema(parameter)" compact :document="document" :navigate-model="navigateModel" />
            <OpenApiMediaDocument :allow-export="allowExport" v-if="isRecord(parameter.content)" :content="parameter.content" :document="document" :navigate-model="navigateModel" />
            <div v-if="examples(parameter).length" class="space-y-2">
              <template v-for="example in examples(parameter)" :key="example.name">
                <div v-if="scalarExample(example.value)" class="openapi-parameter-inline-example">
                  <span>{{ t("specification.openapiExample") }}:</span><code>{{ example.value }}</code>
                </div>
                <details v-else class="overflow-hidden rounded-md border border-border">
                  <summary class="cursor-pointer bg-muted/25 px-3 py-2 text-xs text-muted-foreground">{{ t("specification.openapiExample") }} · {{ example.label }}</summary>
                  <p v-if="example.description" class="px-3 pt-2 text-xs text-muted-foreground">{{ example.description }}</p>
                  <OpenApiCodeSample :text="formatOpenApiExample(example.value, typeof example.value === 'object' ? 'application/json' : 'text/plain')" :language="typeof example.value === 'object' ? 'json' : 'text'" />
                </details>
              </template>
            </div>
            <div v-if="serialization(parameter).length" class="flex flex-wrap gap-2 text-[11px] text-muted-foreground">
              <code v-for="rule in serialization(parameter)" :key="rule" class="rounded border border-border px-1.5 py-0.5" translate="no">{{ rule }}</code>
            </div>
          </dd>
        </div>
      </dl>
    </section>
  </div>
</template>
