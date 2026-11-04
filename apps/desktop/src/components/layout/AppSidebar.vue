<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowDownUp, Download, Upload, ChevronsDownUp, FolderPlus, RefreshCw, ChevronsLeft, Search, X, Plus } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import LightDropdown from "@/components/ui/LightDropdown.vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import SidebarLocateButton from "@/components/sidebar/SidebarLocateButton.vue";
import SidebarListOptionsIcon from "@/components/sidebar/SidebarListOptionsIcon.vue";
import ResourceTree from "@/components/sidebar/ResourceTree.vue";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
import type { ResourceTreeNode } from "@/types/navigation";
import { useToast } from "@/composables/useToast";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useCreateQueueStore } from "@/stores/createQueueStore";
import { translate } from "@/i18n";

defineProps<{ sidebarWidth: number }>();
const emit = defineEmits<{ import: []; export: []; startResize: [event: PointerEvent]; collapse: []; remove: [node: ResourceTreeNode] }>();
const navigation = useNavigationStore();
const settings = useSettingsStore();
const workspace = useWorkspaceStore();
const { toast } = useToast();
const t = translate;
const shortcuts = useShortcutStore();
const creations = useCreateQueueStore();
const parentSaving = computed(() => creations.current.some((item) => item.id === navigation.selectedId && item.kind === "group"));
const searchInput = ref<HTMLInputElement>();
const transferItems = computed(() => [
  { value: "import", label: t("sidebar.import"), icon: Download },
  { value: "export", label: t("sidebar.export"), icon: Upload, separatorBefore: true },
]);
const options = computed(() => [
  { value: "sort:manual", label: t("sidebar.manualSort") },
  { value: "sort:asc", label: t("sidebar.nameAsc") },
  { value: "sort:desc", label: t("sidebar.nameDesc") },
  { value: "tag:", label: t("sidebar.allTags"), separatorBefore: true },
  ...(workspace.catalog?.tags ?? []).map((tag) => ({ value: `tag:${tag.id}`, label: t("sidebar.tag", { name: tag.name }) })),
  { value: "expand", label: t("sidebar.expandAll"), separatorBefore: true },
  { value: "clear", label: t("sidebar.clearFilters") },
]);
function applyOption(value: string) {
  if (value.startsWith("sort:")) settings.navigation.sortMode = value.slice(5) as "manual" | "asc" | "desc";
  else if (value.startsWith("tag:")) navigation.tagId = value.slice(4);
  else if (value === "expand") navigation.expandAll();
  else {
    navigation.query = "";
    navigation.tagId = "";
  }
}
function focusSearch() {
  searchInput.value?.focus();
  searchInput.value?.select();
}
function createGroup() {
  const selected = workspace.catalog?.groups.find((group) => group.id === navigation.selectedId);
  navigation.beginCreate("group", selected?.id ?? null);
}
async function refresh() {
  try {
    await workspace.refresh();
  } catch (error) {
    toast(error instanceof Error ? error.message : t("sidebar.refreshFailed"), 5000);
  }
}
defineExpose({ focusSearch });
</script>

<template>
  <!-- Toolbar actions, independent tree surface, and resize handle. -->
  <aside data-app-sidebar class="app-sidebar-panel h-full shrink-0 relative select-none" :class="{ 'specx-panel': settings.appLayout === 'separated' }" :style="{ width: sidebarWidth + 'px' }">
    <div class="h-full flex flex-col overflow-hidden rounded-[inherit]">
      <div class="app-sidebar-toolbar flex items-center gap-px px-3 text-xs font-medium text-muted-foreground border-b bg-muted/20" :class="settings.appLayout === 'classic' ? 'h-9' : 'h-10'">
        <LightDropdown
          model-value=""
          :items="transferItems"
          :aria-label="t('sidebar.importExport')"
          :trigger-label="t('sidebar.importExport')"
          :trigger-icon="ArrowDownUp"
          trigger-class="inline-flex h-7 min-w-0 items-center gap-1 rounded-md px-1 hover:bg-muted hover:text-foreground"
          trigger-icon-class="h-3.5 w-3.5 shrink-0"
          content-class="w-48"
          :highlight-selected="false"
          check-position="none"
          @update:model-value="(value) => (value === 'import' ? emit('import') : emit('export'))"
        />
        <span class="flex-1 self-stretch" />
        <AppTooltip :text="t('sidebar.collapseAll')"
          ><Button variant="ghost" size="icon" class="h-5 w-5" :aria-label="t('sidebar.collapseAll')" @click="navigation.collapseAll"><ChevronsDownUp class="h-3 w-3" /></Button
        ></AppTooltip>
        <AppTooltip :text="t('sidebar.createGroup')"
          ><Button variant="ghost" size="icon" class="h-5 w-5" :aria-label="t('sidebar.createGroup')" :disabled="navigation.busy || parentSaving" @click="createGroup"><FolderPlus class="h-3 w-3" /></Button
        ></AppTooltip>
        <AppTooltip :text="t('sidebar.refresh')"
          ><Button variant="ghost" size="icon" class="h-5 w-5" :aria-label="t('sidebar.refresh')" @click="refresh"><RefreshCw class="h-3 w-3" /></Button
        ></AppTooltip>
        <AppTooltip :text="shortcuts.hint(t('sidebar.collapse'), 'sidebar')"
          ><Button variant="ghost" size="icon" class="h-6 w-6" :aria-label="shortcuts.hint(t('sidebar.collapse'), 'sidebar')" @click="emit('collapse')"><ChevronsLeft class="h-3.5 w-3.5" /></Button
        ></AppTooltip>
      </div>
      <div class="resource-tree-search flex items-center gap-1 px-2 py-1">
        <div class="flex h-6 min-w-0 flex-1 items-center gap-1 rounded border border-border bg-background px-1.5">
          <Search class="h-3 w-3 text-muted-foreground" /><input ref="searchInput" v-model="navigation.query" :aria-label="t('sidebar.search')" :placeholder="t('sidebar.search')" class="min-w-0 flex-1 bg-transparent text-xs outline-none" /><button
            v-if="navigation.query"
            type="button"
            :aria-label="t('sidebar.clearSearch')"
            @click="navigation.query = ''"
          >
            <X class="h-3 w-3" />
          </button>
        </div>
        <SidebarLocateButton :label="t('sidebar.locate')" @locate="navigation.locateActive" />
        <LightDropdown
          model-value=""
          :items="options"
          :aria-label="t('sidebar.sortFilter')"
          :show-trigger-label="false"
          :show-chevron="false"
          trigger-class="flex h-6 w-6 items-center justify-center rounded border border-border text-muted-foreground hover:bg-accent"
          content-class="w-52"
          check-position="left"
          :selected-values="[`sort:${settings.navigation.sortMode}`, `tag:${navigation.tagId}`]"
          align="end"
          @update:model-value="applyOption"
        >
          <template #trigger-icon="{ open }"><SidebarListOptionsIcon :filtered="Boolean(navigation.query || navigation.tagId)" :open="open" /></template>
        </LightDropdown>
      </div>
      <div v-if="navigation.tagId" class="px-3 pb-1 text-[10px] text-muted-foreground">
        {{ t("sidebar.tag", { name: workspace.catalog?.tags.find((tag) => tag.id === navigation.tagId)?.name ?? "" }) }}<button class="ml-2 underline" @click="navigation.tagId = ''">{{ t("sidebar.clear") }}</button>
      </div>
      <ResourceTree @remove="emit('remove', $event)" />
      <div class="flex items-center justify-between border-t border-border px-2 py-1 text-[11px] text-muted-foreground">
        <span>{{ t("sidebar.serviceCount", { count: workspace.catalog?.services.length ?? 0 }) }}</span
        ><Button variant="ghost" size="sm" class="h-6 text-xs" :disabled="navigation.busy || parentSaving" @click="navigation.beginCreate('resource', workspace.catalog?.groups.some((group) => group.id === navigation.selectedId) ? navigation.selectedId : null)"
          ><Plus class="h-3 w-3" />{{ t("sidebar.createService") }}</Button
        >
      </div>
    </div>
    <div class="panel-resize-handle panel-resize-handle--right" @pointerdown="emit('startResize', $event)" />
  </aside>
</template>
