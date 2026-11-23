<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { MinusCircle } from "@lucide/vue";
import OpenApiParameterTypeSelect from "../../shared/OpenApiParameterTypeSelect.vue";
import OpenApiParameterValueEditor from "./OpenApiParameterValueEditor.vue";
import { debugFieldSchema, type DebugFormRow } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";

export type { DebugFormRow } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";
const props = defineProps<{ multipart: boolean; document?: OpenApiObject | null }>();
const rows = defineModel<DebugFormRow[]>({ required: true });
const root = ref<HTMLElement>();
const allEnabled = computed(() => rows.value.length > 0 && rows.value.every((row) => row.enabled));
const partiallyEnabled = computed(() => !allEnabled.value && rows.value.some((row) => row.enabled));
function patch(index: number, value: Partial<DebugFormRow>) {
  rows.value = rows.value.map((row, i) => (i === index ? { ...row, ...value } : row));
}
function changeType(index: number, type: string) {
  const isFile = props.multipart && type === "file";
  patch(index, { type, isFile, schema: undefined, files: [], ...(rows.value[index]?.isFile || isFile ? { value: "" } : {}) });
}
async function add(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.value) return;
  rows.value = [...rows.value, { name: input.value, value: "", enabled: true }];
  input.value = "";
  await nextTick();
  root.value?.querySelector<HTMLInputElement>(`[name="openapi-form-name-${rows.value.length - 1}"]`)?.focus();
}
</script>
<template>
  <div ref="root" class="debug-parameter-table debug-form-table" role="region" :aria-label="t('specification.openapiRequestBody')" tabindex="0">
    <table>
      <caption class="sr-only">
        {{
          t("specification.openapiRequestBody")
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
          <th scope="col"><input type="checkbox" :checked="allEnabled" :indeterminate="partiallyEnabled" :disabled="!rows.length" :aria-label="t('specification.openapiSelectAllParameters')" @change="rows = rows.map((row) => ({ ...row, enabled: ($event.target as HTMLInputElement).checked }))" /></th>
          <th scope="col">{{ t("specification.openapiParameterNameLabel") }}</th>
          <th scope="col">{{ t("specification.openapiParameterValueLabel") }}</th>
          <th scope="col">{{ t("specification.openapiParameterType") }}</th>
          <th scope="col">{{ t("specification.openapiDescription") }}</th>
          <th scope="col" />
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in rows" :key="index" class="debug-parameter-row" :class="{ 'is-disabled': !row.enabled }">
          <td><input :checked="row.enabled" type="checkbox" :aria-label="t('specification.openapiEnable')" :name="`openapi-form-enabled-${index}`" @change="patch(index, { enabled: ($event.target as HTMLInputElement).checked })" /></td>
          <td>
            <input
              :value="row.name"
              class="field-control"
              :placeholder="t('specification.openapiFormFieldName')"
              :aria-label="t('specification.openapiFormFieldName')"
              :name="`openapi-form-name-${index}`"
              autocomplete="off"
              @input="patch(index, { name: ($event.target as HTMLInputElement).value })"
            />
          </td>
          <td>
            <input
              v-if="multipart && row.isFile"
              class="field-control"
              type="file"
              :multiple="row.multiple"
              :aria-label="t('specification.openapiFormFileValue')"
              :name="`openapi-form-file-${index}`"
              @change="patch(index, { files: Array.from(($event.target as HTMLInputElement).files ?? []), value: '' })"
            />
            <OpenApiParameterValueEditor
              v-else
              :model-value="row.value"
              :document="document"
              :schema="row.schema ?? debugFieldSchema(row.type || 'string', document)"
              :name="`openapi-form-value-${index}`"
              :label="row.name"
              :placeholder="t('specification.openapiFormFieldValue')"
              @update:model-value="patch(index, { value: $event })"
            />
          </td>
          <td>
            <OpenApiParameterTypeSelect :model-value="row.isFile ? 'file' : row.type || 'string'" :document="document" :file="multipart" :schema="row.schema" @configuration="patch(index, $event)" :name="`openapi-form-type-${index}`" @update:model-value="changeType(index, $event)" />
          </td>
          <td>
            <input class="field-control" :value="row.description || ''" :aria-label="t('specification.openapiDescription')" :placeholder="row.required ? t('specification.openapiRequired') : ''" @input="patch(index, { description: ($event.target as HTMLInputElement).value })" />
          </td>
          <td>
            <button type="button" class="icon-button" :aria-label="t('common.delete')" @click="rows = rows.filter((_, i) => i !== index)"><MinusCircle class="size-3.5" /></button>
          </td>
        </tr>
        <tr class="debug-parameter-add-row">
          <td />
          <td><input class="field-control" :placeholder="t('specification.openapiAddParameter')" :aria-label="t('specification.openapiAddParameter')" @input="add" /></td>
          <td colspan="4" />
        </tr>
      </tbody>
    </table>
  </div>
</template>
