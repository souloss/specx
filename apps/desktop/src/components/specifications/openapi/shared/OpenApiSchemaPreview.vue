<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowUpRight, Check, Clipboard, Layers3 } from "@lucide/vue";
import { isRecord, mergeOpenApiSchema, openApiSchemaName } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { openApiModelForReference } from "@/lib/specifications/openapi/contract/openapiModels";
import { openApiPointerValue } from "@/lib/specifications/openapi/contract/openapiReferences";
import { translate } from "@/i18n";

const props = withDefaults(defineProps<{ schema: unknown; document: OpenApiObject | null; depth?: number; mode?: "summary" | "full"; compact?: boolean; navigateModel?: (reference: string) => void; ancestorReferences?: string[] }>(), {
  depth: 0,
  mode: "summary",
  compact: false,
  ancestorReferences: () => [],
});
const t = translate;
const fieldsExpanded = ref(true);
const rawExpanded = ref(false);
const copied = ref(false);
const schema = computed(() => mergeOpenApiSchema(props.document, props.schema));
const name = computed(() => modelTarget.value?.name ?? openApiSchemaName(props.schema));
const sourceRef = computed(() => (isRecord(props.schema) && typeof props.schema.$ref === "string" ? props.schema.$ref : ""));
const modelTarget = computed(() => (sourceRef.value ? openApiModelForReference(props.document, sourceRef.value) : undefined));
const circular = computed(() => Boolean(sourceRef.value && props.ancestorReferences.includes(modelTarget.value?.reference ?? sourceRef.value)));
const childReferences = computed(() => (sourceRef.value ? [...props.ancestorReferences, modelTarget.value?.reference ?? sourceRef.value] : props.ancestorReferences));
const booleanSchema = computed(() => (typeof props.schema === "boolean" ? props.schema : sourceRef.value && props.document ? openApiPointerValue(props.document, sourceRef.value) : undefined));
const type = computed(() => {
  if (typeof booleanSchema.value === "boolean") return t(booleanSchema.value ? "specification.openapiModels.anyValue" : "specification.openapiModels.noValue");
  const value = schema.value;
  if (!value) return t("specification.openapiUnspecified");
  if (Array.isArray(value.type)) return value.type.join(" | ");
  if (value.items) {
    const item = mergeOpenApiSchema(props.document, value.items);
    const itemType = openApiSchemaName(value.items) ?? (typeof item?.type === "string" ? item.type : item?.properties ? t("specification.openapiSchemaObject") : t("specification.openapiSchemaItem"));
    return `${t("specification.openapiSchemaArray")}<${itemType}>`;
  }
  if (value.type === "object" && value.additionalProperties === true) return "Record<string, unknown>";
  if (value.type === "object" && isRecord(value.additionalProperties)) return `Record<string, ${propertyType(value.additionalProperties)}>`;
  if (typeof value.type === "string") return typeof value.format === "string" ? `${value.type} · ${value.format}` : value.type;
  if (value.properties) return t("specification.openapiSchemaObject");
  return t("specification.openapiSchema");
});
const properties = computed(() => (isRecord(schema.value?.properties) ? Object.entries(schema.value.properties) : []));
const variants = computed(() => {
  const value = schema.value;
  for (const key of ["oneOf", "anyOf", "allOf"] as const) if (Array.isArray(value?.[key])) return { key, items: value[key].filter(isRecord) };
  return null;
});
const items = computed(() => (schema.value?.items ? schema.value.items : null));
const additionalProperties = computed(() => (isRecord(schema.value?.additionalProperties) ? schema.value?.additionalProperties : null));
// A summary should reduce decoration, not hide the contract. Keep traversing
// nested objects so a referenced response never degrades to only `object`.
const showNestedFields = computed(() => props.depth < 8 && !circular.value);
const example = computed(() => schema.value?.example ?? (Array.isArray(schema.value?.examples) ? schema.value.examples[0] : undefined));
const defaultValue = computed(() => schema.value?.default);
const enumValues = computed(() => (Array.isArray(schema.value?.enum) ? schema.value.enum : []));
const rawSchema = computed(() => (typeof booleanSchema.value === "boolean" ? String(booleanSchema.value) : schema.value ? JSON.stringify(schema.value, null, 2) : ""));
const constraints = computed(() => {
  const value = schema.value;
  if (!value) return [];
  const entries: string[] = [];
  if (typeof value.minimum === "number") entries.push(`${t(value.exclusiveMinimum === true ? "specification.openapiSchemaExclusiveMinimum" : "specification.openapiSchemaMinimum")} ${value.minimum}`);
  if (typeof value.maximum === "number") entries.push(`${t(value.exclusiveMaximum === true ? "specification.openapiSchemaExclusiveMaximum" : "specification.openapiSchemaMaximum")} ${value.maximum}`);
  if (typeof value.exclusiveMinimum === "number") entries.push(`${t("specification.openapiSchemaExclusiveMinimum")} ${value.exclusiveMinimum}`);
  if (typeof value.exclusiveMaximum === "number") entries.push(`${t("specification.openapiSchemaExclusiveMaximum")} ${value.exclusiveMaximum}`);
  if (typeof value.minLength === "number") entries.push(`${t("specification.openapiSchemaMinLength")} ${value.minLength}`);
  if (typeof value.maxLength === "number") entries.push(`${t("specification.openapiSchemaMaxLength")} ${value.maxLength}`);
  if (typeof value.minItems === "number") entries.push(`${t("specification.openapiSchemaMinItems")} ${value.minItems}`);
  if (typeof value.maxItems === "number") entries.push(`${t("specification.openapiSchemaMaxItems")} ${value.maxItems}`);
  if (typeof value.minProperties === "number") entries.push(`${t("specification.openapiSchemaMinProperties")} ${value.minProperties}`);
  if (typeof value.maxProperties === "number") entries.push(`${t("specification.openapiSchemaMaxProperties")} ${value.maxProperties}`);
  if (value.uniqueItems === true) entries.push(t("specification.openapiSchemaUniqueItems"));
  if (typeof value.pattern === "string") entries.push(`${t("specification.openapiSchemaPattern")} ${value.pattern}`);
  if (value.readOnly === true) entries.push(t("specification.openapiSchemaReadOnly"));
  if (value.writeOnly === true) entries.push(t("specification.openapiSchemaWriteOnly"));
  if (value.additionalProperties === false) entries.push(t("specification.openapiSchemaNoAdditionalProperties"));
  return entries;
});

function propertyType(value: unknown, depth = 0): string {
  const raw = isRecord(value) && typeof value.$ref === "string" && props.document ? openApiPointerValue(props.document, value.$ref) : value;
  if (typeof raw === "boolean") return t(raw ? "specification.openapiModels.anyValue" : "specification.openapiModels.noValue");
  if (depth > 8) return t("specification.openapiSchema");
  const resolved = mergeOpenApiSchema(props.document, value);
  if (!resolved) return t("specification.openapiUnspecified");
  if (Array.isArray(resolved.type)) return resolved.type.join(" | ");
  const nullable = resolved.nullable === true && typeof resolved.type === "string" ? " | null" : "";
  if (resolved.items) {
    const item = mergeOpenApiSchema(props.document, resolved.items);
    const itemType = openApiSchemaName(resolved.items) ?? (typeof item?.type === "string" ? item.type : item?.properties ? t("specification.openapiSchemaObject") : t("specification.openapiSchemaItem"));
    return `${t("specification.openapiSchemaArray")}<${itemType}>${nullable}`;
  }
  if (resolved.type === "object" && resolved.additionalProperties === true) return `Record<string, unknown>${nullable}`;
  if (resolved.type === "object" && isRecord(resolved.additionalProperties)) return `Record<string, ${propertyType(resolved.additionalProperties, depth + 1)}>${nullable}`;
  if (typeof resolved.type === "string") return `${resolved.type}${typeof resolved.format === "string" ? ` · ${resolved.format}` : ""}${nullable}`;
  if (resolved.properties) return t("specification.openapiSchemaObject");
  if (resolved.oneOf || resolved.anyOf || resolved.allOf) return t("specification.openapiSchemaVariant");
  return t("specification.openapiSchema");
}

function propertyDescription(value: unknown): string {
  const resolved = mergeOpenApiSchema(props.document, value);
  return typeof resolved?.description === "string" ? resolved.description : "";
}

function isRequired(key: string): boolean {
  return Array.isArray(schema.value?.required) && schema.value.required.includes(key);
}

async function copyRawSchema() {
  if (!rawSchema.value) return;
  try {
    await navigator.clipboard.writeText(rawSchema.value);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 1600);
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <div class="schema-preview" :class="{ 'schema-preview-nested': props.depth > 0 }">
    <div v-if="!props.compact" class="flex flex-wrap items-center gap-2">
      <Layers3 v-if="name" class="h-3.5 w-3.5 text-primary" aria-hidden="true" />
      <button
        v-if="name && modelTarget && props.navigateModel"
        type="button"
        class="inline-flex items-center gap-1 rounded text-xs font-semibold text-primary hover:underline focus-visible:outline-2 focus-visible:outline-primary"
        :aria-label="t('specification.openapiModels.open', { name })"
        @click="props.navigateModel(modelTarget.reference)"
      >
        <code>{{ name }}</code
        ><ArrowUpRight class="size-3" aria-hidden="true" />
      </button>
      <code v-else-if="name" class="text-xs font-semibold text-primary">{{ name }}</code>
      <span class="schema-type">{{ type }}</span>
      <span v-if="schema?.nullable" class="schema-flag">{{ t("specification.openapiSchemaNullable") }}</span>
    </div>
    <button
      v-if="props.compact && modelTarget && props.navigateModel"
      type="button"
      class="my-1 inline-flex items-center gap-1 rounded text-xs text-primary hover:underline focus-visible:outline-2 focus-visible:outline-primary"
      :aria-label="t('specification.openapiModels.open', { name: modelTarget.name })"
      @click="props.navigateModel(modelTarget.reference)"
    >
      <Layers3 class="size-3" aria-hidden="true" />{{ modelTarget.name }}<ArrowUpRight class="size-3" aria-hidden="true" />
    </button>
    <p v-if="circular" class="mt-2 text-xs text-muted-foreground">{{ t("specification.openapiModels.circular") }}</p>
    <p v-if="sourceRef && !props.compact" class="schema-reference">
      <span>{{ t("specification.openapiSchemaSource") }}</span
      ><code>{{ sourceRef }}</code>
    </p>
    <p v-if="schema?.description && !props.compact" class="mt-1 text-xs leading-5 text-muted-foreground">{{ schema.description }}</p>
    <div v-if="constraints.length" class="schema-constraints" :class="{ 'schema-constraints-compact': props.compact }">
      <span v-for="constraint in constraints" :key="constraint" class="schema-constraint">{{ constraint }}</span>
    </div>
    <div v-if="example !== undefined || defaultValue !== undefined || enumValues.length" class="schema-value-hints" :class="{ 'schema-value-hints-compact': props.compact }">
      <span v-if="example !== undefined" class="schema-value-hint"
        ><b>{{ t("specification.openapiSchemaExample") }}</b
        ><code>{{ typeof example === "object" ? JSON.stringify(example) : String(example) }}</code></span
      >
      <span v-if="defaultValue !== undefined" class="schema-value-hint"
        ><b>{{ t("specification.openapiSchemaDefault") }}</b
        ><code>{{ typeof defaultValue === "object" ? JSON.stringify(defaultValue) : String(defaultValue) }}</code></span
      >
      <span v-if="enumValues.length" class="schema-value-hint"
        ><b>{{ t("specification.openapiSchemaEnum") }}</b
        ><code>{{ enumValues.map((value) => (typeof value === "object" ? JSON.stringify(value) : String(value))).join(" | ") }}</code></span
      >
    </div>
    <div v-if="properties.length && showNestedFields" class="schema-fields" :aria-label="t('specification.openapiFields')">
      <div class="schema-fields-heading">
        <button type="button" class="schema-fields-toggle" :aria-expanded="fieldsExpanded" @click="fieldsExpanded = !fieldsExpanded">
          <span>{{ t("specification.openapiFields") }}</span>
          <span class="schema-field-count">{{ properties.length }}</span>
        </button>
        <button type="button" class="schema-fields-action" @click="fieldsExpanded = !fieldsExpanded">{{ fieldsExpanded ? t("specification.openapiCollapseFields") : t("specification.openapiExpandFields") }}</button>
      </div>
      <div v-if="fieldsExpanded" class="schema-fields-body">
        <div v-for="[key, value] in properties" :key="key" class="schema-field">
          <div class="schema-field-main">
            <code>{{ key }}</code>
            <span v-if="isRequired(key)" class="schema-required">{{ t("specification.openapiRequiredMark") }}</span>
            <span class="schema-field-type">{{ propertyType(value) }}</span>
          </div>
          <p v-if="propertyDescription(value)" class="schema-field-description">{{ propertyDescription(value) }}</p>
          <OpenApiSchemaPreview v-if="showNestedFields && isRecord(value)" :schema="value" :document="document" :depth="props.depth + 1" :mode="props.mode" :navigate-model="props.navigateModel" :ancestor-references="childReferences" compact />
        </div>
      </div>
    </div>
    <div v-if="variants && showNestedFields" class="mt-3 space-y-2">
      <div class="schema-variant-heading">
        {{ variants.key }} <span>{{ variants.items.length }}</span>
      </div>
      <div v-for="(variant, index) in variants.items" :key="index" class="schema-variant">
        <div class="schema-variant-title">
          <span>{{ t("specification.openapiSchemaVariantItem", { index: index + 1 }) }}</span
          ><code v-if="openApiSchemaName(variant)">{{ openApiSchemaName(variant) }}</code>
        </div>
        <OpenApiSchemaPreview :schema="variant" :document="document" :depth="props.depth + 1" :mode="props.mode" :navigate-model="props.navigateModel" :ancestor-references="childReferences" />
      </div>
    </div>
    <div v-if="name && !props.compact && rawSchema" class="schema-source">
      <div class="schema-source-toolbar">
        <button type="button" class="schema-source-toggle" :aria-expanded="rawExpanded" @click="rawExpanded = !rawExpanded">{{ t("specification.openapiSchemaRaw") }}</button>
        <button type="button" class="schema-source-copy" :aria-label="t('specification.openapiCopySchema')" @click="copyRawSchema">
          <Check v-if="copied" class="h-3 w-3" /><Clipboard v-else class="h-3 w-3" />{{ copied ? t("specification.openapiSchemaCopied") : t("specification.openapiCopySchema") }}
        </button>
      </div>
      <pre v-show="rawExpanded" tabindex="0">{{ rawSchema }}</pre>
    </div>
    <div v-if="items && showNestedFields && props.depth < 8" class="mt-2 border-l border-border pl-3">
      <span class="text-[11px] text-muted-foreground"
        >{{ t("specification.openapiItems") }} <code class="ml-1">{{ propertyType(items) }}</code></span
      ><OpenApiSchemaPreview :schema="items" :document="document" :depth="props.depth + 1" :mode="props.mode" :navigate-model="props.navigateModel" :ancestor-references="childReferences" compact />
    </div>
    <div v-if="additionalProperties && showNestedFields && props.depth < 8" class="mt-2 border-l border-border pl-3">
      <span class="text-[11px] text-muted-foreground"
        >{{ t("specification.openapiAdditionalProperties") }} <code class="ml-1">{{ propertyType(additionalProperties) }}</code></span
      ><OpenApiSchemaPreview :schema="additionalProperties" :document="document" :depth="props.depth + 1" :mode="props.mode" :navigate-model="props.navigateModel" :ancestor-references="childReferences" compact />
    </div>
  </div>
</template>
