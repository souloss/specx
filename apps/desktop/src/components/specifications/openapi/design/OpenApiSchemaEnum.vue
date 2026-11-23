<script setup lang="ts">
import { computed, ref } from "vue";
import { Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate as t } from "@/i18n";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { constraintValueText, parseSchemaFieldValue, sameSchemaValue } from "@/lib/specifications/openapi/design/openapiSchemaFields";

const schema = defineModel<OpenApiObject>({ required: true });
const props = defineProps<{ type: string; nullable: boolean }>();
const adding = ref(false);
const newValue = ref("");
const error = ref(false);
const values = computed<unknown[]>(() => (Array.isArray(schema.value.enum) ? schema.value.enum : []));
const metadata = computed(() => (Array.isArray(schema.value["x-apifox-enum"]) ? schema.value["x-apifox-enum"].filter(isRecord) : []));
const entry = (value: unknown) => metadata.value.find((item) => sameSchemaValue(item.value, value));

function save(values: unknown[], rows: OpenApiObject[]) {
  const next = { ...schema.value };
  if (values.length) next.enum = values;
  else delete next.enum;
  if (rows.length) next["x-apifox-enum"] = rows;
  else delete next["x-apifox-enum"];
  schema.value = next;
}

function setMetadata(value: unknown, key: "name" | "description", text: string) {
  const previous = entry(value);
  const next = { value, name: "", description: "", ...previous, [key]: text };
  save(values.value, previous ? metadata.value.map((row) => (row === previous ? next : row)) : [...metadata.value, next]);
}

function change(index: number, text: string) {
  const parsed = parseSchemaFieldValue(text, props.type, props.nullable);
  error.value = !parsed || values.value.some((value, i) => i !== index && sameSchemaValue(value, parsed.value));
  if (error.value || !parsed) return;
  if (index === values.value.length) {
    save([...values.value, parsed.value], [...metadata.value, { value: parsed.value, name: "", description: "" }]);
    newValue.value = "";
  } else {
    const previous = values.value[index];
    save(
      values.value.map((value, i) => (index === i ? parsed.value : value)),
      metadata.value.map((row) => (sameSchemaValue(row.value, previous) ? { ...row, value: parsed.value } : row)),
    );
  }
}
</script>

<template>
  <div class="grid gap-2">
    <label class="schema-constraint-toggle">
      <input
        type="checkbox"
        role="switch"
        :checked="adding || values.length > 0"
        @change="
          (event) => {
            adding = (event.target as HTMLInputElement).checked;
            error = false;
            if (!adding) save([], []);
          }
        "
      />
      {{ t("specification.openapiSchemaEnum") }}
    </label>
    <template v-if="adding || values.length">
      <div v-for="(value, index) in values" :key="index" class="schema-enum-entry">
        <input :value="constraintValueText(value, nullable)" class="field-control field-control-compact" :aria-label="t('specification.openapiSchemaEnumValue')" @change="change(index, ($event.target as HTMLInputElement).value)" />
        <input :value="entry(value)?.name" class="field-control field-control-compact" :aria-label="t('specification.openapiName')" :placeholder="t('specification.openapiName')" @input="setMetadata(value, 'name', ($event.target as HTMLInputElement).value)" />
        <input :value="entry(value)?.description" class="field-control field-control-compact" :aria-label="t('specification.openapiDescription')" :placeholder="t('specification.openapiDescription')" @input="setMetadata(value, 'description', ($event.target as HTMLInputElement).value)" />
        <button
          type="button"
          class="icon-button"
          :aria-label="t('common.delete')"
          @click="
            error = false;
            save(
              values.filter((_, i) => i !== index),
              metadata.filter((row) => !sameSchemaValue(row.value, value)),
            );
          "
        >
          <Trash2 class="size-3.5" aria-hidden="true" />
        </button>
      </div>
      <div class="flex min-w-0 items-center gap-2">
        <input v-model="newValue" class="field-control field-control-compact min-w-0 flex-1" :aria-label="t('specification.openapiSchemaEnumValue')" :placeholder="t('specification.openapiSchemaEnumValue')" @keydown.enter.prevent="change(values.length, newValue)" />
        <Button type="button" size="xs" variant="outline" @click="change(values.length, newValue)"><Plus class="size-3.5" aria-hidden="true" />{{ t("specification.openapiSchemaEnumAdd") }}</Button>
      </div>
      <p v-if="error" class="text-xs text-destructive" role="alert">{{ t("specification.openapiSchemaEnumInvalid") }}</p>
    </template>
  </div>
</template>

<style scoped>
.schema-enum-entry {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr)) auto;
  gap: 0.4rem;
}
</style>
