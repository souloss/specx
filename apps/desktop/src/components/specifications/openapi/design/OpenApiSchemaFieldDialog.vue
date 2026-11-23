<script setup lang="ts">
import { computed, ref, useId } from "vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { translate as t } from "@/i18n";
import { isRecord, resolveOpenApiRef, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { changeSchemaFieldType, schemaFieldType, schemaModelConstraints, schemaModelReference, withSchemaModel } from "@/lib/specifications/openapi/design/openapiSchemaFields";
import OpenApiOperationTabs from "../OpenApiOperationTabs.vue";
import OpenApiSchemaConstraints from "./OpenApiSchemaConstraints.vue";

const props = withDefaults(defineProps<{ schema: OpenApiObject; label: string; fieldName: string; document?: OpenApiObject | null; types: readonly string[]; required?: boolean; requiredLocked?: boolean; deprecated?: boolean }>(), { required: undefined, deprecated: undefined });
const emit = defineEmits<{ apply: [schema: OpenApiObject, required?: boolean, deprecated?: boolean]; close: [] }>();
const schema = ref(props.schema);
const required = ref(props.required);
const deprecated = ref(props.deprecated);
const activeTab = ref("form");
const json = ref("");
const id = useId();
const tabs = computed(() => [
  { value: "form", id: `${id}-form`, label: t("specification.openapiDesignSchemaType") },
  { value: "json", id: `${id}-json`, label: t("specification.openapiDesignJsonSchema") },
]);
const reference = computed(() => schemaModelReference(schema.value));
const type = computed(() => {
  const referenced = reference.value ? resolveOpenApiRef(props.document ?? null, { $ref: reference.value }) : undefined;
  return schemaFieldType(referenced ?? schema.value);
});
const constraints = computed({
  get: () => (reference.value ? schemaModelConstraints(schema.value) : schema.value),
  set: (value) => {
    schema.value = reference.value ? withSchemaModel(value, reference.value, props.document) : value;
  },
});
const parsedJson = computed(() => {
  try {
    const value: unknown = JSON.parse(json.value);
    if (isRecord(value)) return { value };
    return { error: t("specification.openapiDesignSchemaJsonObjectRequired") };
  } catch {
    return { error: t("specification.openapiDesignSchemaJsonInvalid") };
  }
});

function updateTab(tab: string) {
  if (tab === activeTab.value) return;
  if (tab === "json") json.value = JSON.stringify(schema.value, null, 2);
  else if (parsedJson.value.value) schema.value = parsedJson.value.value;
  activeTab.value = tab;
}

function apply() {
  const value = activeTab.value === "json" ? parsedJson.value.value : schema.value;
  if (value) emit("apply", value, required.value, deprecated.value);
}
</script>

<template>
  <Dialog :open="true" @update:open="(open) => !open && emit('close')">
    <DialogScrollContent class="schema-field-dialog grid-cols-[minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 sm:max-w-[540px]">
      <header class="flex justify-center px-10 pt-3">
        <DialogTitle class="sr-only">{{ t("specification.openapiSchemaAdvancedSettings") }} · {{ label }}</DialogTitle>
        <OpenApiOperationTabs :model-value="activeTab" :tabs="tabs" class="schema-field-dialog-tabs" :ariaLabel="t('specification.openapiDesignSchemaViews')" @update:model-value="updateTab" />
      </header>
      <div class="schema-field-dialog-body">
        <div v-if="activeTab === 'form'" role="tabpanel" :aria-labelledby="`${id}-form`" class="grid gap-3">
          <code v-if="reference" class="break-all text-primary">{{ reference }}</code>
          <AppSelect v-else :value="type" size="form" class="w-full" :aria-label="t('specification.openapiType')" @update:model-value="schema = changeSchemaFieldType(schema, $event, document)">
            <option v-if="!types.includes(type)" :value="type" disabled>{{ type || t("specification.openapiSchemaTypeFromComposition") }}</option>
            <option v-for="item in types" :key="item" :value="item">{{ item }}</option>
          </AppSelect>
          <OpenApiSchemaConstraints v-model="constraints" v-model:required="required" v-model:deprecated="deprecated" :type="type" :document="document" :reference="Boolean(reference)" :required-locked="requiredLocked" :field-name="fieldName" />
          <slot name="structure" :schema="schema" :update="(value: OpenApiObject) => (schema = value)" />
          <slot />
        </div>
        <div v-else role="tabpanel" :aria-labelledby="`${id}-json`" class="grid gap-2">
          <textarea v-model="json" class="field-control min-h-60 resize-y font-mono" :aria-label="t('specification.openapiDesignJsonSchema')" :aria-invalid="Boolean(parsedJson.error)" :aria-describedby="parsedJson.error ? `${id}-error` : undefined" spellcheck="false" autocapitalize="off" />
          <p v-if="parsedJson.error" :id="`${id}-error`" class="text-xs text-destructive" role="alert">{{ parsedJson.error }}</p>
        </div>
      </div>
      <DialogFooter class="m-0 border-t border-border px-4 py-3">
        <Button size="sm" type="button" variant="outline" @click="emit('close')">{{ t("common.cancel") }}</Button>
        <Button size="sm" type="button" :disabled="activeTab === 'json' && Boolean(parsedJson.error)" @click="apply">{{ t("common.done") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>

<style>
.schema-field-dialog-body {
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 12px 16px 16px;
}
.schema-field-dialog-tabs.openapi-operation-tabs {
  width: max-content;
  max-width: 100%;
  gap: 2px;
  padding: 2px;
  border: 0;
  border-radius: 8px;
  background: var(--muted);
}
.schema-field-dialog-tabs .openapi-operation-tab {
  border: 1px solid transparent;
  border-radius: 6px;
  min-height: 30px;
  padding: 4px 10px;
}
.schema-field-dialog-tabs .openapi-operation-tab[aria-selected="true"] {
  border-color: var(--border);
  color: var(--foreground);
  background: var(--background);
}
.schema-field-dialog-tabs .openapi-operation-tab::after {
  display: none;
}
</style>
