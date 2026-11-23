<script setup lang="ts">
import { nextTick, ref } from "vue";
import { Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate as t } from "@/i18n";
import type { EnvironmentVariableDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";

defineProps<{ invalidField: string }>();
const rows = defineModel<EnvironmentVariableDraft[]>({ required: true });
const emit = defineEmits<{ dirty: [] }>();
const table = ref<HTMLElement>();
const newName = ref("");

async function add(focusValue: boolean) {
  const key = newName.value.trim();
  if (!key) return;
  const index = rows.value.length;
  rows.value.push({ key, sensitive: false, enabled: true });
  newName.value = "";
  emit("dirty");
  if (focusValue) {
    await nextTick();
    table.value?.querySelector<HTMLInputElement>(`[name="environment-value-${index}"]`)?.focus();
  }
}
</script>

<template>
  <section class="environment-variables-table">
    <h3>{{ t("requestAgent.variables") }}</h3>
    <div ref="table" class="environment-variable-grid">
      <table>
        <colgroup>
          <col class="variable-control" />
          <col class="variable-name" />
          <col />
          <col class="variable-control" />
          <col class="variable-control" />
        </colgroup>
        <thead>
          <tr>
            <th scope="col">
              <span class="sr-only">{{ t("environmentContext.enabled") }}</span>
            </th>
            <th scope="col">{{ t("environmentContext.variableName") }}</th>
            <th scope="col">{{ t("environmentContext.variableValue") }}</th>
            <th scope="col">
              <span class="sr-only">{{ t("environmentContext.secret") }}</span>
            </th>
            <th scope="col">
              <span class="sr-only">{{ t("common.delete") }}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, index) in rows" :key="index">
            <td class="variable-control"><input v-model="row.enabled" type="checkbox" :aria-label="t('environmentContext.enabled')" :title="t('environmentContext.enabled')" /></td>
            <td><input v-model="row.key" class="variable-field" :name="`environment-key-${index}`" :aria-label="t('environmentContext.variableName')" :aria-invalid="invalidField === `environment-key-${index}`" maxlength="120" autocomplete="off" spellcheck="false" /></td>
            <td>
              <input
                class="variable-field"
                :name="`environment-value-${index}`"
                :value="row.localValue ?? row.value ?? ''"
                :type="row.sensitive ? 'password' : 'text'"
                :aria-label="t('environmentContext.variableValue')"
                :placeholder="row.sensitive && row.hasValue ? t('environmentContext.secretRetained') : ''"
                autocomplete="off"
                spellcheck="false"
                @input="row.localValue = ($event.target as HTMLInputElement).value"
              />
            </td>
            <td class="variable-control"><input v-model="row.sensitive" type="checkbox" :aria-label="t('environmentContext.secret')" :title="t('environmentContext.secret')" /></td>
            <td>
              <Button
                size="icon-sm"
                variant="ghost"
                :aria-label="t('common.delete')"
                @click="
                  rows.splice(index, 1);
                  emit('dirty');
                "
                ><Trash2 class="size-3.5"
              /></Button>
            </td>
          </tr>
          <tr v-if="rows.length < 100">
            <td />
            <td><input v-model="newName" class="variable-field" :placeholder="t('environmentContext.addVariable')" :aria-label="t('environmentContext.addVariable')" maxlength="120" autocomplete="off" @blur="add(false)" @keydown.enter.prevent="add(true)" /></td>
            <td colspan="3" />
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
h3 {
  margin-block-end: 12px;
  font-weight: 500;
}
.environment-variable-grid {
  border: 1px solid var(--border);
  border-radius: 8px;
}
table {
  width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: 13px;
}
.variable-name {
  width: 32%;
}
.variable-control {
  width: 29px;
  text-align: center;
}
th {
  padding: 8px 5px;
  text-align: start;
  color: var(--muted-foreground);
  font-weight: 500;
}
td {
  border-block-start: 1px solid var(--border);
  padding: 2px;
}
td:nth-child(2),
td:nth-child(3) {
  border-inline-start: 1px solid var(--border);
}
.variable-field {
  display: block;
  width: 100%;
  min-width: 0;
  height: 30px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  padding: 0 4px;
}
.variable-field:hover,
.variable-field:focus-visible {
  border-color: var(--border);
}
</style>
