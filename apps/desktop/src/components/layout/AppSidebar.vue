<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronsDownUp, FolderPlus, RefreshCw, ChevronsLeft, Search, X, Plus, MoreHorizontal } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import LightDropdown from "@/components/ui/LightDropdown.vue";
import SidebarLocateButton from "@/components/sidebar/SidebarLocateButton.vue";
import SidebarRegexToggleButton from "@/components/sidebar/SidebarRegexToggleButton.vue";
import SidebarListOptionsIcon from "@/components/sidebar/SidebarListOptionsIcon.vue";
import ResourceTree from "@/components/sidebar/ResourceTree.vue";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
import type { ResourceTreeNode } from "@/types/navigation";
import { useToast } from "@/composables/useToast";
import { useCreateQueueStore } from "@/stores/createQueueStore";
import { translate } from "@/i18n";

defineProps<{ sidebarWidth: number }>();
const emit = defineEmits<{ startResize: [event: PointerEvent]; collapse: []; remove: [node: ResourceTreeNode] }>();
const navigation = useNavigationStore();
const settings = useSettingsStore();
const workspace = useWorkspaceStore();
const { toast } = useToast();
const t = translate;
const creations = useCreateQueueStore();
const parentSaving = computed(() => creations.current.some((item) => item.id === navigation.selectedId && item.kind === "group"));
const searchInput = ref<HTMLInputElement>();
const options = computed(() => [
  { value: "sort:manual", label: t("sidebar.manualSort") },
  { value: "sort:asc", label: t("sidebar.nameAsc") },
  { value: "sort:desc", label: t("sidebar.nameDesc") },
  { value: "tag:", label: t("sidebar.allTags"), separatorBefore: true },
  ...(workspace.catalog?.tags ?? []).map((tag) => ({ value: `tag:${tag.id}`, label: t("sidebar.tag", { name: tag.name }) })),
  { value: "expand", label: t("sidebar.expandAll"), separatorBefore: true },
  { value: "clear", label: t("sidebar.clearFilters") },
]);
const actionOptions = computed(() => [
  { value: "collapse-all", label: t("sidebar.collapseAll"), icon: ChevronsDownUp },
  { value: "create-group", label: t("sidebar.createGroup"), icon: FolderPlus },
  { value: "refresh", label: t("sidebar.refresh"), icon: RefreshCw },
  { value: "collapse-sidebar", label: t("sidebar.collapse"), icon: ChevronsLeft },
]);
function applyOption(value: string) {
  if (value.startsWith("sort:")) settings.navigation.sortMode = value.slice(5) as "manual" | "asc" | "desc";
  else if (value.startsWith("tag:")) navigation.tagId = value.slice(4);
  else if (value === "expand") navigation.expandAll();
  else {
    navigation.query = "";
    navigation.regexMode = false;
    navigation.tagId = "";
  }
}
function applyToolbarAction(value: string) {
  if (value === "collapse-all") navigation.collapseAll();
  else if (value === "create-group") createGroup();
  else if (value === "refresh") void refresh();
  else if (value === "collapse-sidebar") emit("collapse");
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
      <div class="app-sidebar-toolbar flex min-w-0 items-center gap-1 border-b bg-muted/20 px-2 text-xs font-medium text-muted-foreground" :class="settings.appLayout === 'classic' ? 'h-9' : 'h-10'">
        <div class="flex h-7 min-w-0 flex-1 items-center rounded border bg-background px-1.5" :class="navigation.regexInvalid ? 'border-destructive focus-within:ring-1 focus-within:ring-destructive' : 'border-border focus-within:ring-1 focus-within:ring-ring'">
          <Search class="h-3.5 w-3.5 shrink-0 text-muted-foreground" /><input
            ref="searchInput"
            v-model="navigation.query"
            :aria-label="t('sidebar.search')"
            :aria-invalid="navigation.regexInvalid"
            :placeholder="t('sidebar.search')"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            class="min-w-0 flex-1 bg-transparent px-1.5 text-xs outline-none placeholder:text-muted-foreground"
          /><button v-if="navigation.query" type="button" :aria-label="t('sidebar.clearSearch')" @click="navigation.query = ''">
            <X class="h-3 w-3" />
          </button>
          <SidebarRegexToggleButton :label="t('sidebar.regexSearch')" :pressed="navigation.regexMode" :invalid="navigation.regexInvalid" @toggle="navigation.regexMode = !navigation.regexMode" />
        </div>
        <SidebarLocateButton :label="t('sidebar.locate')" @locate="navigation.locateActive" />
        <LightDropdown
          model-value=""
          :items="options"
          :aria-label="t('sidebar.sortFilter')"
          :show-trigger-label="false"
          :show-chevron="false"
          trigger-class="flex h-7 w-7 items-center justify-center rounded border border-border text-muted-foreground hover:bg-accent"
          content-class="w-52"
          check-position="left"
          :selected-values="[`sort:${settings.navigation.sortMode}`, `tag:${navigation.tagId}`]"
          align="end"
          @update:model-value="applyOption"
        >
          <template #trigger-icon="{ open }"><SidebarListOptionsIcon :filtered="Boolean(navigation.query || navigation.tagId || navigation.regexMode)" :open="open" /></template>
        </LightDropdown>
        <LightDropdown
          model-value=""
          :items="actionOptions"
          :aria-label="t('sidebar.moreActions')"
          :show-trigger-label="false"
          :show-chevron="false"
          trigger-class="flex h-7 w-7 shrink-0 items-center justify-center rounded border border-border text-muted-foreground hover:bg-accent"
          content-class="w-48"
          check-position="none"
          align="end"
          @update:model-value="applyToolbarAction"
        >
          <template #trigger-icon><MoreHorizontal class="h-3.5 w-3.5" /></template>
        </LightDropdown>
      </div>
      <p v-if="navigation.regexInvalid" class="px-3 pb-1 text-[10px] text-destructive">{{ t("sidebar.invalidRegex") }}</p>
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
