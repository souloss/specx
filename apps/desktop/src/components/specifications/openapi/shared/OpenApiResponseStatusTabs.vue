<script setup lang="ts">
import OpenApiOperationTabs, { type OpenApiOperationTab } from "../OpenApiOperationTabs.vue";

defineProps<{ tabs: OpenApiOperationTab[]; ariaLabel: string }>();
const selected = defineModel<string>({ required: true });

function statusClass(code: string) {
  if (code.startsWith("2")) return "is-success";
  if (code.startsWith("4") || code.startsWith("5")) return "is-error";
  return "is-warning";
}
</script>

<template>
  <OpenApiOperationTabs v-model="selected" class="openapi-response-status-tabs" :tabs="tabs" :ariaLabel="ariaLabel">
    <template #tab="{ tab }">
      <span class="openapi-response-status-code" :class="statusClass(tab.status ?? tab.value)" translate="no">{{ tab.status ?? tab.value }}</span>
      <span v-if="tab.label" class="openapi-response-tab-label" :title="tab.label">{{ tab.label }}</span>
    </template>
  </OpenApiOperationTabs>
</template>
