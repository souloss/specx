<script setup lang="ts">
import { ref } from "vue";
import { Check, ChevronDown } from "@lucide/vue";
import AppPopover from "@/components/ui/AppPopover.vue";
import { translate as t } from "@/i18n";

export type RequestEnvironmentChoice = { environmentId: string; serverId: string | null; label: string; url: string };
defineProps<{ choices: RequestEnvironmentChoice[]; environmentId: string | null; serverId: string | null; baseUrl: string; ignored: boolean; disabled: boolean }>();
const emit = defineEmits<{ select: [choice: RequestEnvironmentChoice] }>();
const open = ref(false);

function select(choice: RequestEnvironmentChoice) {
  emit("select", choice);
  open.value = false;
}
</script>

<template>
  <AppPopover v-model:open="open" align="start" content-class="request-environment-popover">
    <template #trigger>
      <button type="button" class="request-environment-trigger" :class="{ ignored }" :disabled="disabled" :aria-label="t('environmentContext.switchBaseUrl', { url: baseUrl || t('requestAgent.baseUrl') })" :title="baseUrl || t('requestAgent.baseUrl')">
        <span>{{ baseUrl || t("requestAgent.baseUrl") }}</span
        ><ChevronDown class="size-3 shrink-0" aria-hidden="true" />
      </button>
    </template>
    <p class="request-environment-hint">{{ t("environmentContext.baseUrlHelp") }}</p>
    <ul class="request-environment-choices">
      <li v-for="choice in choices" :key="`${choice.environmentId}:${choice.serverId}`">
        <button type="button" :aria-pressed="choice.environmentId === environmentId && choice.serverId === serverId" @click="select(choice)">
          <span class="request-environment-check"><Check v-if="choice.environmentId === environmentId && choice.serverId === serverId" class="size-3" aria-hidden="true" /></span>
          <span class="request-environment-url">{{ choice.url || choice.label }}</span>
          <span v-if="choice.url" class="request-environment-name">{{ choice.label }}</span>
        </button>
      </li>
    </ul>
  </AppPopover>
</template>

<style scoped>
.request-environment-trigger {
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  min-width: 0;
  max-width: 55%;
  gap: 5px;
  margin-inline: 2px 0;
  padding: 3px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--background);
  color: var(--muted-foreground);
  font-size: 12px;
}
.request-environment-trigger span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.request-environment-trigger:hover,
.request-environment-trigger[data-state="open"] {
  background: var(--accent);
  color: var(--accent-foreground);
}
.request-environment-trigger.ignored span {
  text-decoration: line-through;
}
.request-environment-hint {
  padding: 10px 12px;
  border-block-end: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 12px;
}
.request-environment-choices {
  padding: 4px;
}
.request-environment-choices button {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) minmax(0, auto);
  align-items: center;
  width: 100%;
  gap: 7px;
  min-height: 32px;
  padding: 5px 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  font-size: 13px;
  text-align: start;
}
.request-environment-choices button:hover,
.request-environment-choices button[aria-pressed="true"] {
  border-color: var(--border);
  background: var(--surface-muted);
}
.request-environment-check {
  display: grid;
  place-items: center;
  width: 15px;
  height: 15px;
  border: 1px solid var(--border);
  border-radius: 50%;
}
[aria-pressed="true"] .request-environment-check {
  border-color: var(--primary);
  background: var(--primary);
  color: var(--primary-foreground);
}
.request-environment-url,
.request-environment-name {
  overflow-wrap: anywhere;
}
.request-environment-name {
  color: var(--muted-foreground);
  text-align: end;
  font-size: 12px;
  max-width: 12ch;
}
</style>
<style>
.app-popover.request-environment-popover {
  width: min(40rem, var(--reka-popover-content-available-width), calc(100vw - 24px));
  padding: 0;
}
</style>
