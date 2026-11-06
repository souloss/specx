<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { X, FileText, Box, Settings } from "@lucide/vue";
import CustomContextMenu from "@/components/ui/CustomContextMenu.vue";
import { useTabScroll } from "@/composables/useTabScroll";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settingsStore";
import { translate } from "@/i18n";
const workspace = useWorkspaceStore();
const settings = useSettingsStore();
const t = translate;
const tabsRef = ref<HTMLElement | null>(null);
const { hasTabOverflow, scrollThumbLeftPercent, scrollThumbWidthPercent, updateScrollButtons, onTabsWheel, startScrollbarDrag } = useTabScroll(tabsRef);
watch(
  () => [workspace.tabs.length, workspace.activeTabId],
  async () => {
    await nextTick();
    tabsRef.value?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
    updateScrollButtons();
  },
);
function activate(id: string) {
  settings.showSettingsPage = false;
  workspace.activeTabId = id;
}
function closeTabs(ids: string[]) {
  for (const id of ids) if (!workspace.closeTab(id)) break;
}
function keyboard(event: KeyboardEvent) {
  const index = workspace.tabs.findIndex((tab) => tab.id === workspace.activeTabId);
  const direction = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
  if (!direction || !workspace.tabs.length) return;
  event.preventDefault();
  activate(workspace.tabs[(index + direction + workspace.tabs.length) % workspace.tabs.length].id);
  void nextTick(() => tabsRef.value?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
}
</script>

<template>
  <div class="app-tab-bar relative shrink-0 border-b border-border bg-muted/20" :class="{ 'classic-tab-layout': settings.appLayout === 'classic' }">
    <div ref="tabsRef" role="tablist" :aria-label="t('tabs.ariaLabel')" class="app-tab-scroll flex h-9 min-w-0 items-center overflow-x-auto [scrollbar-width:none]" @wheel="onTabsWheel" @keydown="keyboard">
      <CustomContextMenu
        v-for="tab in workspace.tabs"
        :key="tab.id"
        :items="[
          { label: t('tabs.close'), action: () => workspace.closeTab(tab.id) },
          {
            label: t('tabs.closeOthers'),
            action: () => closeTabs(workspace.tabs.filter((item) => item.id !== tab.id).map((item) => item.id)),
          },
          {
            label: t('tabs.closeAll'),
            action: () => closeTabs(workspace.tabs.map((item) => item.id)),
          },
        ]"
      >
        <template #default="{ onContextMenu }"
          ><div
            role="tab"
            :aria-selected="workspace.activeTabId === tab.id && !settings.showSettingsPage"
            :tabindex="workspace.activeTabId === tab.id ? 0 : -1"
            class="app-tab-pill group"
            :class="{ active: workspace.activeTabId === tab.id && !settings.showSettingsPage }"
            @click="activate(tab.id)"
            @keydown.enter.prevent="activate(tab.id)"
            @contextmenu="onContextMenu"
            @auxclick="$event.button === 1 && workspace.closeTab(tab.id)"
          >
            <component :is="tab.kind === 'specification' ? FileText : Box" class="h-3 w-3 shrink-0" /><span class="max-w-44 truncate">{{ tab.title }}</span
            ><button type="button" class="ml-2 rounded p-0.5 hover:bg-muted" :aria-label="t('tabs.closeTab', { name: tab.title })" @click.stop="workspace.closeTab(tab.id)"><X class="h-3 w-3" /></button></div
        ></template>
      </CustomContextMenu>
      <div v-if="settings.showSettingsPage" role="tab" aria-selected="true" class="app-tab-pill active">
        <Settings class="h-3 w-3" />{{ t("common.settings") }}<button :aria-label="t('tabs.closeSettings')" @click="settings.showSettingsPage = false"><X class="h-3 w-3" /></button>
      </div>
    </div>
    <div v-if="hasTabOverflow" class="absolute inset-x-0 bottom-0 h-1 cursor-pointer bg-muted" @pointerdown="startScrollbarDrag"><div class="h-full rounded-full bg-muted-foreground/40" :style="{ marginLeft: `${scrollThumbLeftPercent}%`, width: `${scrollThumbWidthPercent}%` }" /></div>
  </div>
</template>
