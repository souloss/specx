<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { ChevronDown, Plus, SlidersHorizontal, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppPopover from "@/components/ui/AppPopover.vue";
import OpenApiRequestProxyOptions from "../workbench/request/OpenApiRequestProxyOptions.vue";
import { createId } from "@/lib/core";
import { translate as t } from "@/i18n";
import type { EnvironmentDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";
import type { OpenApiEnvironmentServer } from "@/types/api";

defineProps<{ invalidField: string }>();
const emit = defineEmits<{ dirty: [] }>();
const draft = defineModel<EnvironmentDraft>({ required: true });
const table = ref<HTMLElement>();
const rows = computed(() => [{ id: "", name: t("requestAgent.defaultModule"), baseUrl: draft.value.baseUrl, requestMode: draft.value.requestMode }, ...(draft.value.servers ?? [])]);

function change(row: OpenApiEnvironmentServer, key: "baseUrl" | "requestMode", value: string) {
  const mode = value === "browser" || value === "agent" ? value : "inherit";
  if (row.id) {
    if (key === "baseUrl") row.baseUrl = value;
    else row.requestMode = mode;
  } else if (key === "baseUrl") draft.value.baseUrl = value;
  else draft.value.requestMode = mode;
  emit("dirty");
}

async function add() {
  const id = createId("server");
  draft.value.servers = [...(draft.value.servers ?? []), { id, name: "", baseUrl: "", requestMode: "inherit" }];
  emit("dirty");
  await nextTick();
  table.value?.querySelector<HTMLInputElement>(`[name="environment-server-name-${id}"]`)?.focus();
}

function remove(id: string) {
  draft.value.servers = draft.value.servers?.filter((row) => row.id !== id);
  emit("dirty");
}
</script>

<template>
  <section class="environment-server-section">
    <div class="server-section-heading">
      <h3>{{ t("requestAgent.baseUrl") }}</h3>
      <Button variant="ghost" size="sm" :disabled="rows.length > 50" @click="add"><Plus class="size-4" />{{ t("environmentContext.addServer") }}</Button>
    </div>
    <div ref="table" class="environment-server-table">
      <table>
        <colgroup>
          <col class="server-col-name" />
          <col />
          <col class="server-col-actions" />
        </colgroup>
        <thead>
          <tr>
            <th scope="col">{{ t("environmentContext.server") }}</th>
            <th scope="col">{{ t("requestAgent.baseUrl") }}</th>
            <th scope="col">
              <span class="sr-only">{{ t("environmentContext.removeServer") }}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in rows" :key="row.id">
            <td>
              <input
                v-if="row.id"
                v-model="row.name"
                class="server-field"
                :name="`environment-server-name-${row.id}`"
                :placeholder="t('environmentContext.serverName')"
                :aria-label="t('environmentContext.serverName')"
                :aria-invalid="invalidField === `environment-server-name-${row.id}`"
                maxlength="80"
                autocomplete="off"
              />
              <span v-else class="server-default">{{ row.name }}</span>
            </td>
            <td :colspan="row.id ? 1 : 2">
              <div class="server-url-editor">
                <input
                  class="server-field"
                  :name="row.id ? `environment-server-url-${row.id}` : 'environment-base-url'"
                  :value="row.baseUrl"
                  placeholder="https://api.example.com"
                  :aria-label="t('requestAgent.baseUrl')"
                  :aria-invalid="invalidField === (row.id ? `environment-server-url-${row.id}` : 'environment-base-url')"
                  maxlength="2000"
                  autocomplete="off"
                  autocapitalize="none"
                  spellcheck="false"
                  @input="change(row, 'baseUrl', ($event.target as HTMLInputElement).value)"
                />
                <AppPopover align="end">
                  <template #trigger
                    ><button type="button" class="environment-server-advanced" :aria-label="t('requestAgent.advanced')" :title="t('requestAgent.advanced')">
                      <SlidersHorizontal class="size-3.5" /><span>{{ t("requestAgent.advanced") }}</span
                      ><ChevronDown class="size-3" /></button
                  ></template>
                  <OpenApiRequestProxyOptions :model-value="row.requestMode" environment @update:model-value="change(row, 'requestMode', $event)" />
                </AppPopover>
              </div>
            </td>
            <td v-if="row.id">
              <Button size="icon-sm" variant="ghost" :aria-label="t('environmentContext.removeServer')" @click="remove(row.id)"><Trash2 class="size-3.5" /></Button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.environment-server-section {
  container: environment-server / inline-size;
}
.server-section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-block-end: 6px;
}
.server-section-heading h3 {
  font-weight: 500;
}
.environment-server-table {
  border: 1px solid var(--border);
  border-radius: 8px;
}
table {
  width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: 13px;
}
.server-col-name {
  width: min(32%, 180px);
}
.server-col-actions {
  width: 32px;
}
th {
  padding: 8px 10px;
  color: var(--muted-foreground);
  font-weight: 500;
  text-align: start;
}
td {
  border-block-start: 1px solid var(--border);
  padding: 2px;
}
th + th,
td + td {
  border-inline-start: 1px solid var(--border);
}
.server-default {
  display: block;
  overflow-wrap: anywhere;
  padding: 4px 8px;
}
.server-url-editor {
  display: flex;
  align-items: center;
  min-width: 0;
}
.server-field {
  display: block;
  width: 100%;
  min-width: 0;
  height: 30px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  padding: 0 7px;
}
.server-field:hover,
.server-field:focus-visible {
  border-color: var(--border);
}
.server-field:focus-visible {
  background: var(--background);
}
.environment-server-advanced {
  display: flex;
  align-items: center;
  flex: none;
  gap: 5px;
  padding: 4px 7px;
  border-radius: 4px;
  color: var(--muted-foreground);
  font-size: 12px;
}
.environment-server-advanced:hover,
.environment-server-advanced[data-state="open"] {
  background: var(--accent);
  color: var(--accent-foreground);
}
@media (hover: hover) {
  tr:not(:focus-within, :hover) .environment-server-advanced:not([data-state="open"]) {
    opacity: 0;
  }
}
@container environment-server (max-width: 420px) {
  .environment-server-advanced span,
  .environment-server-advanced svg:last-child {
    display: none;
  }
}
</style>
