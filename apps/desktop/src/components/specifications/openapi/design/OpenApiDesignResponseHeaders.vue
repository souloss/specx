<script setup lang="ts">
import { Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate as t } from "@/i18n";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import type { ResponseHeaderDraft } from "@/lib/specifications/openapi/design/openapiDesignDraft";
import OpenApiSchemaFieldType from "./OpenApiSchemaFieldType.vue";

const props = defineProps<{ rows?: ResponseHeaderDraft[]; document: OpenApiObject | null; editable: boolean; invalidKeys: string[] }>();
const emit = defineEmits<{ update: [rows: ResponseHeaderDraft[]] }>();

function add() {
  emit("update", [...(props.rows ?? []), { key: crypto.randomUUID(), name: "", definition: { schema: { type: "string" } }, editable: true }]);
}

function schema(row: ResponseHeaderDraft) {
  return isRecord(row.definition.schema) ? row.definition.schema : { type: "string" };
}

function description(row: ResponseHeaderDraft, event: Event) {
  const value = (event.target as HTMLInputElement).value;
  if (value) row.definition.description = value;
  else delete row.definition.description;
}
</script>

<template>
  <details v-if="editable || rows?.length" class="openapi-design-response-headers">
    <summary>
      <Plus v-if="!rows?.length" class="size-3.5" aria-hidden="true" />{{ t("specification.openapiResponseHeaders") }} <span v-if="rows?.length" class="text-muted-foreground">({{ rows.length }})</span>
    </summary>
    <div v-for="row in rows" :key="row.key" class="openapi-design-header-entry">
      <input
        v-model="row.name"
        class="field-control"
        :aria-label="t('specification.openapiName')"
        :placeholder="t('specification.openapiName')"
        :aria-invalid="props.invalidKeys.includes(row.key)"
        :aria-describedby="props.invalidKeys.includes(row.key) ? 'openapi-design-response-header-error' : undefined"
        :disabled="!editable || !row.editable"
      />
      <input :value="row.definition.description" class="field-control" :aria-label="t('specification.openapiDescription')" :placeholder="t('specification.openapiDescription')" :disabled="!editable || !row.editable" @input="description(row, $event)" />
      <OpenApiSchemaFieldType
        v-if="row.editable"
        :model-value="schema(row)"
        :required="row.definition.required === true"
        :name="`response-header-type-${row.key}`"
        :field-name="`response-header-${row.key}`"
        :label="row.name"
        :document="document"
        :disabled="!editable"
        @update:model-value="row.definition.schema = $event"
        @update:required="row.definition.required = $event"
      />
      <span v-else class="text-xs text-muted-foreground">{{ t("specification.openapiDesignReferenceReadonly") }}</span>
      <button
        v-if="editable"
        type="button"
        class="icon-button"
        :aria-label="t('specification.openapiDesignRemoveParameter', { name: row.name })"
        @click="
          emit(
            'update',
            (rows ?? []).filter((item) => item.key !== row.key),
          )
        "
      >
        <Trash2 class="size-3.5" />
      </button>
    </div>
    <Button v-if="editable" size="xs" variant="ghost" @click="add"><Plus class="size-3.5" />{{ t("specification.openapiDesignAddResponseHeader") }}</Button>
  </details>
</template>
