<script setup lang="ts">
import { computed, watch } from "vue";
import { Settings2 } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import { Button } from "@/components/ui/button";
import OpenApiEnvironmentManager from "@/components/specifications/openapi/environments/OpenApiEnvironmentManager.vue";
import { translate as t } from "@/i18n";
import { useEnvironmentContextStore } from "@/stores/environmentContextStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
const context = useEnvironmentContextStore();
const settings = useSettingsStore();
const workspace = useWorkspaceStore();
const tab = computed(() => workspace.activeTab);
const activeFormat = computed(() => {
  if (settings.showSettingsPage) return null;
  if (tab.value?.kind === "provider") return tab.value.providerKind;
  if (tab.value?.kind === "specification") return workspace.catalog?.specifications.find((item) => item.id === tab.value?.specificationId)?.kind;
  return null;
});
const serviceId = computed(() => (activeFormat.value === "openapi" ? (tab.value?.serviceId ?? null) : null));
const provider = computed(() => (serviceId.value ? workspace.openApiProviderFor(serviceId.value) : null));
watch(
  serviceId,
  (id) => {
    context.settingsOpen = false;
    context.activateSource(id ? `openapi:${id}` : null);
    if (id && !workspace.openApiProviderFor(id)) void workspace.loadOpenApiProvider(id).catch(() => {});
  },
  { immediate: true },
);
watch(
  [serviceId, provider],
  ([id, value]) => {
    if (id && value)
      context.registerOptions(
        `openapi:${id}`,
        value.environments.map((item) => ({ id: item.id, name: item.name, scope: "openapi" })),
        value.settings.defaultEnvironmentId,
      );
  },
  { immediate: true },
);
</script>
<template>
  <div class="environment-context-control">
    <AppSelect :model-value="serviceId ? (context.activeId ?? '') : ''" class="environment-context-select" name="active-environment" :disabled="!serviceId || !context.activeOptions.length" :aria-label="t('environmentContext.select')" @update:model-value="context.select(String($event))">
      <option v-if="!serviceId || !context.activeOptions.length" value="" disabled>{{ t("environmentContext.unavailable") }}</option>
      <option v-for="item in serviceId ? context.activeOptions : []" :key="item.id" :value="item.id">{{ item.name }}</option>
    </AppSelect>
    <Button variant="ghost" size="icon" class="environment-context-menu" :disabled="!serviceId || !provider" :aria-label="t('environmentContext.settings')" @click="context.openSettings"><Settings2 class="size-4" aria-hidden="true" /></Button>
  </div>
  <OpenApiEnvironmentManager v-if="serviceId && provider" :key="serviceId" :service-id="serviceId" />
</template>
<style scoped>
.environment-context-control {
  display: flex;
  min-width: 0;
  flex: 0 0 auto;
  align-items: center;
  gap: 2px;
  padding-inline: 8px;
}
.environment-context-select {
  width: min(13rem, 24vw);
  min-width: 7rem;
  flex: 0 1 13rem;
}
.environment-context-menu {
  flex: 0 0 auto;
}
</style>
