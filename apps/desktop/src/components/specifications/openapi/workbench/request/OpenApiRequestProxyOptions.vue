<script setup lang="ts">
import { computed, useId } from "vue";
import { translate as t } from "@/i18n";
import type { RequestProxyMode, EnvironmentProxyMode } from "@/lib/specifications/openapi/workbench/request/openapiRequestAgent";
const props = defineProps<{ environment?: boolean }>();
const mode = defineModel<RequestProxyMode | EnvironmentProxyMode>({ required: true });
const group = useId();
const options = computed(() => [props.environment ? "inherit" : "auto", "browser", "agent"] as const);
</script>
<template>
  <div class="request-proxy-options" role="radiogroup" :aria-label="t('requestAgent.title')">
    <label v-for="option in options" :key="option" class="request-proxy-option" :class="{ selected: mode === option }"
      ><span
        ><strong>{{ t(`requestAgent.${option}`) }}</strong
        ><small>{{ t(`requestAgent.${option}Help`) }}</small></span
      ><input v-model="mode" type="radio" :name="group" :value="option"
    /></label>
  </div>
</template>
