<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { MinusCircle } from "@lucide/vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import OpenApiParameterValueEditor from "./OpenApiParameterValueEditor.vue";
import OpenApiParameterTypeSelect from "../../shared/OpenApiParameterTypeSelect.vue";
import { debugFieldSchema } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiParameterConfiguration, WorkbenchRow } from "@/lib/specifications/openapi/workbench/openapiWorkbench";
import { translate as t } from "@/i18n";

export type DebugParameter = OpenApiParameterConfiguration & { key: string; name: string; type: string; description: string; schema: OpenApiObject; placeholder: string; value: string; enabled: boolean; required: boolean; generated?: boolean; managed?: boolean };
const props = withDefaults(defineProps<{ parameters: DebugParameter[]; section: "path" | "query" | "header" | "cookie"; editable?: boolean; document?: OpenApiObject | null }>(), { editable: true });
const rows = defineModel<WorkbenchRow[]>("rows", { default: () => [] });
const emit = defineEmits<{ value: [key: string, value: string]; enabled: [key: string, enabled: boolean]; configuration: [key: string, value: OpenApiParameterConfiguration]; metadata: [key: string, field: "type" | "description" | "name", value: string] }>();
const table = ref<HTMLElement>();
const prefix = computed(() => (props.section === "header" ? "custom-header" : props.section));
const entries = computed(() => [...props.parameters, ...rows.value]);
const allEnabled = computed(() => entries.value.length > 0 && entries.value.every((row) => row.enabled));
const partiallyEnabled = computed(() => !allEnabled.value && entries.value.some((row) => row.enabled));
const label = computed(() => t(props.section === "path" ? "specification.openapiPathParameters" : props.section === "query" ? "specification.openapiQueryParameters" : props.section === "header" ? "specification.openapiHeaders" : "specification.openapiCookies"));
function toggleAll(enabled: boolean) {
  props.parameters.filter((row) => !row.managed).forEach((row) => emit("enabled", row.key, enabled));
  rows.value = rows.value.map((row) => ({ ...row, enabled }));
}
function configureRow(index: number, value: OpenApiParameterConfiguration) {
  rows.value = rows.value.map((row, i) => (i === index ? { ...row, ...value } : row));
}
function structured(schema: OpenApiObject) {
  return schema.type === "object" || schema.type === "array" || Boolean(schema.properties);
}
function updateRow(index: number, field: keyof WorkbenchRow, value: unknown) {
  rows.value = rows.value.map((row, i) => (i === index ? { ...row, [field]: value, ...(field === "type" ? { schema: undefined } : {}) } : row));
}
async function addRow(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.value) return;
  rows.value = [...rows.value, { name: input.value, value: "", enabled: true }];
  input.value = "";
  await nextTick();
  table.value?.querySelector<HTMLInputElement>(`input[name="openapi-${prefix.value}-name-${rows.value.length - 1}"]`)?.focus();
}
</script>

<template>
  <div ref="table" class="debug-parameter-table" tabindex="0" role="region" :aria-label="label">
    <table>
      <caption class="sr-only">
        {{
          label
        }}
      </caption>
      <colgroup>
        <col class="debug-column-enabled" />
        <col class="debug-column-name" />
        <col class="debug-column-value" />
        <col class="debug-column-type" />
        <col />
        <col class="debug-column-action" />
      </colgroup>
      <thead>
        <tr>
          <th scope="col"><input type="checkbox" :checked="allEnabled" :indeterminate="partiallyEnabled" :disabled="!entries.length" :aria-label="t('specification.openapiSelectAllParameters')" @change="toggleAll(($event.target as HTMLInputElement).checked)" /></th>
          <th scope="col">{{ t("specification.openapiParameterNameLabel") }}</th>
          <th scope="col">{{ t("specification.openapiParameterValueLabel") }}</th>
          <th scope="col">{{ t("specification.openapiParameterType") }}</th>
          <th scope="col">{{ t("specification.openapiDescription") }}</th>
          <th scope="col" />
        </tr>
      </thead>
      <tbody>
        <template v-for="parameter in parameters" :key="parameter.key"
          ><tr class="debug-parameter-row" :class="{ 'is-disabled': !parameter.enabled, 'debug-generated-parameter': parameter.generated }">
            <td>
              <input
                type="checkbox"
                :checked="parameter.enabled"
                :disabled="parameter.managed"
                :title="parameter.managed ? parameter.description : undefined"
                :aria-label="`${t('specification.openapiEnable')} ${parameter.name}`"
                @change="emit('enabled', parameter.key, ($event.target as HTMLInputElement).checked)"
              />
            </td>
            <td class="debug-parameter-name">
              <AppTooltip v-if="section === 'path' && !parameter.generated" :text="t('specification.openapiPathNameHelp')"><input class="field-control" :value="parameter.name" readonly :aria-label="t('specification.openapiParameterName')" /></AppTooltip
              ><input v-else class="field-control" :readonly="parameter.generated" :value="parameter.name" :aria-label="t('specification.openapiParameterName')" @input="emit('metadata', parameter.key, 'name', ($event.target as HTMLInputElement).value)" />
            </td>
            <td class="debug-parameter-input">
              <input v-if="parameter.generated" class="field-control" :value="parameter.value" readonly :aria-label="parameter.name" :title="parameter.value" /><OpenApiParameterValueEditor
                v-else-if="!structured(parameter.schema)"
                :document="document"
                :model-value="parameter.value"
                :schema="parameter.schema"
                :name="`openapi-${parameter.key}`"
                :label="parameter.name"
                :placeholder="parameter.placeholder"
                @update:model-value="emit('value', parameter.key, $event)"
              />
            </td>
            <td class="debug-parameter-type" :data-type="parameter.type">
              <span v-if="parameter.generated" class="debug-generated-type">string</span
              ><OpenApiParameterTypeSelect
                v-else
                :document="document"
                :schema="parameter.schema"
                :location="section"
                :style="parameter.style"
                :explode="parameter.explode"
                @configuration="emit('configuration', parameter.key, $event)"
                :primitive-only="section === 'path'"
                :model-value="parameter.type"
                :name="`openapi-${parameter.key}-type`"
                @update:model-value="emit('metadata', parameter.key, 'type', $event)"
              />
            </td>
            <td class="debug-parameter-description">
              <input class="field-control" :value="parameter.description" :readonly="parameter.generated" :title="parameter.description" :aria-label="t('specification.openapiDescription')" @input="emit('metadata', parameter.key, 'description', ($event.target as HTMLInputElement).value)" />
            </td>
            <td />
          </tr>
          <tr v-if="!parameter.generated && structured(parameter.schema)" class="debug-structured-detail-row">
            <td />
            <td colspan="5"><OpenApiParameterValueEditor :model-value="parameter.value" :schema="parameter.schema" :document="document" :name="`openapi-${parameter.key}`" :label="parameter.name" @update:model-value="emit('value', parameter.key, $event)" /></td></tr
        ></template>
        <template v-for="(row, index) in rows" :key="index"
          ><tr class="debug-parameter-row debug-custom-parameter" :class="{ 'is-disabled': !row.enabled }">
            <td><input type="checkbox" :checked="row.enabled" :name="`openapi-${prefix}-enabled-${index}`" :aria-label="t('specification.openapiEnable')" @change="updateRow(index, 'enabled', ($event.target as HTMLInputElement).checked)" /></td>
            <td>
              <input
                :value="row.name"
                class="field-control"
                :name="`openapi-${prefix}-name-${index}`"
                :placeholder="t('specification.openapiParameterName')"
                :aria-label="t('specification.openapiParameterName')"
                spellcheck="false"
                autocomplete="off"
                @input="updateRow(index, 'name', ($event.target as HTMLInputElement).value)"
              />
            </td>
            <td>
              <OpenApiParameterValueEditor
                v-if="!structured(row.schema ?? debugFieldSchema(row.type || 'string', document))"
                :document="document"
                :model-value="row.value"
                :schema="row.schema ?? debugFieldSchema(row.type || 'string', document)"
                :name="`openapi-${prefix}-value-${index}`"
                :label="row.name"
                :placeholder="t('specification.openapiParameterValue')"
                @update:model-value="updateRow(index, 'value', $event)"
              />
            </td>
            <td class="debug-parameter-type" :data-type="row.type || 'string'">
              <OpenApiParameterTypeSelect
                :document="document"
                :schema="row.schema"
                :location="section"
                :style="row.style"
                :explode="row.explode"
                @configuration="configureRow(index, $event)"
                :primitive-only="section === 'path'"
                :model-value="row.type || 'string'"
                :name="`openapi-${prefix}-type-${index}`"
                @update:model-value="updateRow(index, 'type', $event)"
              />
            </td>
            <td><input class="field-control" :value="row.description || ''" :aria-label="t('specification.openapiDescription')" @input="updateRow(index, 'description', ($event.target as HTMLInputElement).value)" /></td>
            <td>
              <button type="button" class="icon-button" :aria-label="t('common.delete')" @click="rows = rows.filter((_, i) => i !== index)"><MinusCircle class="size-3.5" /></button>
            </td>
          </tr>
          <tr v-if="structured(row.schema ?? debugFieldSchema(row.type || 'string', document))" class="debug-structured-detail-row">
            <td />
            <td colspan="5">
              <OpenApiParameterValueEditor :model-value="row.value" :schema="row.schema ?? debugFieldSchema(row.type || 'string', document)" :document="document" :name="`openapi-${prefix}-value-${index}`" :label="row.name" @update:model-value="updateRow(index, 'value', $event)" />
            </td></tr
        ></template>
        <tr v-if="editable" class="debug-parameter-add-row">
          <td />
          <td>
            <input class="field-control" :placeholder="t('specification.openapiAddParameter')" :aria-label="t('specification.openapiAddParameter')" autocomplete="off" @input="addRow" />
          </td>
          <td colspan="4" />
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.debug-structured-detail-row > td {
  padding: 4px 12px !important;
  background: color-mix(in srgb, var(--surface-muted) 40%, transparent);
}
</style>
