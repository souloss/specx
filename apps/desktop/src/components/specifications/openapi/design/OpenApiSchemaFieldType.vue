<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { Settings2 } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import { translate as t } from "@/i18n";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { changeSchemaFieldType, schemaFieldType, schemaFieldTypes, schemaModelConstraints, schemaModelReference, withSchemaModel } from "@/lib/specifications/openapi/design/openapiSchemaFields";
import OpenApiSchemaConstraints from "./OpenApiSchemaConstraints.vue";
import OpenApiSchemaFieldDialog from "./OpenApiSchemaFieldDialog.vue";
import OpenApiSchemaModelPicker from "../shared/OpenApiSchemaModelPicker.vue";

type FieldSerialization = { style: string; explode: string; allowReserved: boolean };

const schema = defineModel<OpenApiObject>({ required: true });
const required = defineModel<boolean | undefined>("required", { default: undefined });
const deprecated = defineModel<boolean | undefined>("deprecated", { default: undefined });
const props = defineProps<{ name: string; fieldName: string; label: string; document?: OpenApiObject | null; disabled?: boolean; requiredLocked?: boolean; primitiveOnly?: boolean; serialization?: FieldSerialization }>();
const emit = defineEmits<{ serialization: [value: FieldSerialization] }>();
const settings = ref(false);
const serializationDraft = ref<FieldSerialization>();
const picker = ref(false);
const settingsButton = ref<HTMLButtonElement>();
const pickerConstraints = ref<OpenApiObject>({});
const pickerRequired = ref<boolean>();
const pickerDeprecated = ref<boolean>();
const reference = computed(() => schemaModelReference(schema.value));
const types = computed<readonly string[]>(() => (props.primitiveOnly ? schemaFieldTypes.filter((type) => type !== "object" && type !== "array") : schemaFieldTypes));
const type = computed(() => reference.value || schemaFieldType(schema.value));
const typeLabel = computed(() => (reference.value ? reference.value.split("/").at(-1) : type.value || t("specification.openapiSchemaTypeFromComposition")));

function choose(value: string) {
  if (value === "__model") {
    pickerConstraints.value = reference.value ? schemaModelConstraints(schema.value) : Object.fromEntries(Object.entries(schema.value).filter(([key]) => ["description", "title", "deprecated", "readOnly", "writeOnly"].includes(key) || (key.startsWith("x-") && key !== "x-apifox-enum")));
    pickerRequired.value = required.value;
    pickerDeprecated.value = deprecated.value;
    picker.value = true;
  } else if (value !== type.value) schema.value = changeSchemaFieldType(schema.value, value, props.document);
}

function openSettings() {
  const value = props.serialization;
  serializationDraft.value = value ? { style: value.style, explode: value.explode, allowReserved: value.allowReserved } : undefined;
  settings.value = true;
}

function closeSettings() {
  settings.value = false;
  void nextTick(() => settingsButton.value?.focus());
}

async function apply(value: OpenApiObject, nextRequired?: boolean, nextDeprecated?: boolean) {
  if (JSON.stringify(schema.value) !== JSON.stringify(value)) {
    schema.value = value;
    await nextTick();
  }
  if (nextRequired !== undefined && !props.requiredLocked) required.value = nextRequired;
  if (nextDeprecated !== undefined) deprecated.value = nextDeprecated;
}

function selectModel(value: string) {
  apply(withSchemaModel(pickerConstraints.value, value, props.document), pickerRequired.value, pickerDeprecated.value);
}
</script>

<template>
  <div class="schema-field-type">
    <AppSelect :value="type" :id="`${name}-trigger`" :name="name" :disabled="disabled" :aria-label="t('specification.openapiType') + ' ' + label" size="dense" variant="quiet" class="schema-field-select" :data-schema-type="reference ? 'reference' : type" @update:model-value="choose">
      <option v-if="!types.includes(type)" :value="type" disabled>{{ typeLabel }}</option>
      <option v-for="item in types" :key="item" :value="item">{{ item }}</option>
      <option v-if="!primitiveOnly" value="__model">{{ t("specification.openapiDynamic.reference") }}</option>
    </AppSelect>
    <div class="schema-field-actions" role="group" :aria-label="t('specification.openapiDesignSchemaCommonSettings')">
      <AppTooltip v-if="required !== undefined" :text="t('specification.openapiRequired')">
        <button type="button" class="schema-field-action schema-field-required" :aria-pressed="required" :aria-label="t('specification.openapiRequired')" :disabled="disabled || requiredLocked" @click="required = !required">
          <span aria-hidden="true">*</span>
        </button>
      </AppTooltip>
      <AppTooltip :text="t('specification.openapiSchemaAdvancedSettings')">
        <button ref="settingsButton" type="button" class="schema-field-action" :aria-label="t('specification.openapiSchemaAdvancedSettings')" aria-haspopup="dialog" :disabled="disabled" @click="openSettings">
          <Settings2 class="size-3.5" aria-hidden="true" />
        </button>
      </AppTooltip>
    </div>
    <OpenApiSchemaFieldDialog
      v-if="settings && !disabled"
      :schema="schema"
      :label="label"
      :field-name="fieldName"
      :document="document"
      :types="types"
      :required="required"
      :required-locked="requiredLocked"
      :deprecated="deprecated"
      @close="closeSettings"
      @apply="
        (value, nextRequired, nextDeprecated) => {
          apply(value, nextRequired, nextDeprecated);
          if (serializationDraft) emit('serialization', serializationDraft);
          closeSettings();
        }
      "
    >
      <template #structure="slotProps"><slot name="structure" v-bind="slotProps" /></template>
      <slot v-if="serializationDraft" name="serialization" :value="serializationDraft" />
    </OpenApiSchemaFieldDialog>
    <OpenApiSchemaModelPicker v-if="picker && !disabled" v-model:open="picker" :document="document ?? null" :current-reference="reference" :label="label" @select="selectModel">
      <template #constraints="{ model }">
        <div class="mt-4 border-t border-border pt-3">
          <OpenApiSchemaConstraints v-model="pickerConstraints" v-model:required="pickerRequired" v-model:deprecated="pickerDeprecated" :type="isRecord(model.schema) ? schemaFieldType(model.schema) : ''" :document="document" reference :required-locked="requiredLocked" :field-name="fieldName" />
        </div>
      </template>
    </OpenApiSchemaModelPicker>
  </div>
</template>

<style>
.schema-field-type,
.schema-field-actions {
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
}
.schema-field-type {
  min-width: 0;
  max-width: 100%;
  gap: 4px;
}
.schema-field-select.app-select {
  justify-content: start;
  flex-shrink: 1;
}
.schema-field-select > svg {
  display: none;
}
.schema-field-select.app-select[data-schema-type="string"] {
  color: var(--success, #15803d);
}
.schema-field-select.app-select[data-schema-type="number"],
.schema-field-select.app-select[data-schema-type="integer"] {
  color: var(--method-patch, var(--primary));
}
.schema-field-select.app-select[data-schema-type="object"],
.schema-field-select.app-select[data-schema-type="array"],
.schema-field-select.app-select[data-schema-type="reference"] {
  color: var(--primary);
}
.schema-field-actions {
  flex: none;
  border-radius: 5px;
  background: var(--muted);
}
.schema-field-action {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 5px;
  color: var(--muted-foreground);
}
.schema-field-action:hover:not(:disabled) {
  background: var(--accent);
  color: var(--foreground);
}
.schema-field-action:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 1px;
}
.schema-field-action:disabled {
  cursor: default;
}
.schema-field-required {
  font-size: 16px;
  font-weight: 700;
  line-height: 1;
}
.schema-field-required[aria-pressed="true"] {
  color: var(--destructive);
}
</style>
