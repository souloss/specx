<script setup lang="ts">
import { computed, ref } from "vue";
import { Globe, ChevronDown } from "@lucide/vue";
import AppPopover from "@/components/ui/AppPopover.vue";
import OpenApiRequestProxyOptions from "./OpenApiRequestProxyOptions.vue";
import { useRequestAgentStore } from "@/stores/requestAgentStore";
import type { EnvironmentProxyMode } from "@/lib/specifications/openapi/workbench/request/openapiRequestAgent";
import { translate as t } from "@/i18n";
const props = defineProps<{ environmentMode?: EnvironmentProxyMode }>();
const store = useRequestAgentStore();
const open = ref(false);
const label = computed(() => (props.environmentMode && props.environmentMode !== "inherit" ? props.environmentMode : store.mode));
</script>
<template>
  <footer class="openapi-workbench-statusbar">
    <AppPopover v-model:open="open" side="top"
      ><template #trigger
        ><button type="button" class="request-proxy-trigger" :aria-label="t('requestAgent.title')">
          <Globe class="size-3.5" />{{ t("requestAgent.title") }}<span>{{ t(`requestAgent.${label}`) }}</span
          ><ChevronDown class="size-3" /></button
      ></template>
      <div class="request-proxy-popover openapi-workbench-surface">
        <h4>{{ t("requestAgent.title") }}</h4>
        <OpenApiRequestProxyOptions v-model="store.mode" />
        <p v-if="environmentMode && environmentMode !== 'inherit'">{{ t("requestAgent.environmentOverride") }}</p>
      </div></AppPopover
    >
  </footer>
</template>
