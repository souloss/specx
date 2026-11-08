<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { ChevronRight, FileText, Folder, FolderOpen, FolderPlus, Plus, Pencil, Trash2, Box, FolderInput, LoaderCircle, RotateCcw, X } from "@lucide/vue";
import CustomContextMenu from "@/components/ui/CustomContextMenu.vue";
import type { ContextMenuItem } from "@/components/ui/customContextMenuRegistry";
import { useNavigationStore } from "@/stores/navigationStore";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settingsStore";
import { useDragSort } from "@/composables/useDragSort";
import { useToast } from "@/composables/useToast";
import { resourceGroupDestinationRows } from "@/lib/sidebar/sidebarLayout";
import type { ResourceTreeNode } from "@/types/navigation";
import { displayGroupName } from "@/lib/core";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useCreateQueueStore } from "@/stores/createQueueStore";
import { translate } from "@/i18n";

const emit = defineEmits<{ remove: [node: ResourceTreeNode]; createService: [parentId: string | null]; editService: [serviceId: string] }>();
const navigation = useNavigationStore();
const workspace = useWorkspaceStore();
const settings = useSettingsStore();
const shortcuts = useShortcutStore();
const creations = useCreateQueueStore();
const creationById = computed(() => new Map(creations.current.map((item) => [item.id, item])));
const { toast } = useToast();
const t = translate;
const root = ref<HTMLElement | null>(null);
const searchActive = computed(() => !!navigation.query.trim() || !!navigation.tagId);
const canDrag = computed(() => !searchActive.value && !navigation.naming && !navigation.busy && !creations.current.length && settings.navigation.sortMode === "manual");
const {
  state: drag,
  startDrag,
  updateTarget,
  clearTarget,
} = useDragSort((source, target, position) => {
  void navigation.drop(source, target, position).catch(reportError);
});
const rows = computed(() => {
  const result: Array<{ node: ResourceTreeNode; depth: number; expanded: boolean }> = [];
  function visit(nodes: ResourceTreeNode[], depth: number) {
    for (const node of nodes) {
      const expanded = searchActive.value || navigation.expandedIds.has(node.id);
      result.push({ node, depth, expanded });
      if (expanded) visit(node.children ?? [], depth + 1);
    }
  }
  visit(navigation.tree, 0);
  return result;
});
function reportError(error: unknown) {
  toast(error instanceof Error ? error.message : t("tree.operationFailed"), 5000);
}
async function commit() {
  try {
    await navigation.commitName();
  } catch (error) {
    reportError(error);
  }
}
function nameInput(element: unknown) {
  if (element instanceof HTMLInputElement && document.activeElement !== element)
    void nextTick(() => {
      element.focus();
      element.select();
    });
}
async function reveal() {
  await nextTick();
  const element = Array.from(root.value?.querySelectorAll<HTMLElement>("[data-tree-node]") ?? []).find((item) => item.dataset.treeNode === navigation.selectedId);
  element?.scrollIntoView?.({ block: "nearest" });
  element?.focus({ preventScroll: true });
}
watch(() => navigation.revealRevision, reveal);
function menuItems(node: ResourceTreeNode): ContextMenuItem[] {
  const creation = creationById.value.get(node.id);
  if (creation)
    return creation.status === "saving"
      ? []
      : [
          {
            label: t("common.retry"),
            icon: RotateCcw,
            action: () => {
              void creations.retry(node.id);
            },
          },
          { label: t("creation.discard"), icon: X, action: () => creations.discard(node.id) },
        ];
  const group = node.type === "resource-group";
  if (node.type === "directory" || node.type === "leaf") {
    if (node.specificationId) return [{ label: t("tree.openSpecification"), icon: node.providerIcon ?? FileText, action: () => navigation.select(node) }];
    return [{ label: t(node.children?.length && navigation.expandedIds.has(node.id) ? "tree.collapseNode" : "tree.expandNode", { name: node.label }), icon: node.providerIcon ?? Folder, action: () => navigation.toggle(node.id) }];
  }
  const items: ContextMenuItem[] = group
    ? [
        { label: t("tree.createSubgroup"), icon: FolderPlus, action: () => navigation.beginCreate("group", node.id) },
        { label: t("tree.createService"), icon: Plus, action: () => emit("createService", node.id) },
        { label: navigation.expandedIds.has(node.id) ? t("tree.collapseGroup") : t("tree.expandGroup"), action: () => navigation.toggle(node.id) },
      ]
    : [
        { label: t("tree.openService"), icon: Box, action: () => navigation.select(node) },
        { label: t("tree.editService"), icon: Pencil, action: () => emit("editService", node.id) },
        {
          label: t("tree.moveToGroup"),
          icon: FolderInput,
          children: [
            ...resourceGroupDestinationRows(navigation.layout).map((destination) => ({
              label: displayGroupName(destination.id, destination.name),
              title: destination.path.join(" / "),
              indentLevel: destination.depth,
              disabled: creationById.value.has(destination.id),
              action: () => {
                void navigation.moveResource(node.id, destination.id).catch(reportError);
              },
            })),
            {
              label: t("common.ungrouped"),
              action: () => {
                void navigation.moveResource(node.id, null).catch(reportError);
              },
            },
          ],
        },
      ];
  return [
    ...items,
    { separator: true, label: "" },
    { label: t("tree.rename"), icon: Pencil, shortcut: shortcuts.bindings.rename, action: () => navigation.beginRename(node) },
    { label: t("tree.delete"), icon: Trash2, variant: "destructive", disabled: group && !!node.children?.length, title: group && node.children?.length ? t("tree.cannotDeleteGroup") : undefined, action: () => emit("remove", node) },
  ];
}
function onKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || navigation.naming || (event.target instanceof Element && event.target.closest('button, input, textarea, [contenteditable="true"]'))) return;
  const index = rows.value.findIndex((row) => row.node.id === navigation.selectedId);
  const current = rows.value[index];
  if (event.repeat && (shortcuts.matches("rename", event) || shortcuts.matches("delete", event))) return;
  if (current && creationById.value.has(current.node.id) && (shortcuts.matches("rename", event) || shortcuts.matches("delete", event))) return;
  let target = index;
  if (current && shortcuts.matches("rename", event)) navigation.beginRename(current.node);
  else if (current && shortcuts.matches("delete", event) && !current.node.children?.length) emit("remove", current.node);
  else if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
  else if (event.key === "ArrowDown") target = Math.min(rows.value.length - 1, index + 1);
  else if (event.key === "ArrowUp") target = Math.max(0, index - 1);
  else if (event.key === "Home") target = 0;
  else if (event.key === "End") target = rows.value.length - 1;
  else if (event.key === "ArrowRight" && current?.node.children?.length) {
    if (!current.expanded) navigation.toggle(current.node.id);
    else target = Math.min(rows.value.length - 1, index + 1);
  } else if (event.key === "ArrowLeft" && current) {
    if (current.node.children?.length && current.expanded) navigation.toggle(current.node.id);
    else target = rows.value.findIndex((row) => row.node.id === current.node.parentId);
  } else if (event.key === "Enter" && current) navigation.select(current.node);
  else return;
  event.preventDefault();
  if (rows.value[target]) navigation.selectedId = rows.value[target].node.id;
  void reveal();
}
defineExpose({ reveal });
</script>

<template>
  <div ref="root" class="resource-tree-scroller flex-1 min-h-0 overflow-auto py-1" role="tree" :aria-label="t('tree.ariaLabel')" tabindex="0" :aria-busy="navigation.busy" @keydown="onKeydown">
    <CustomContextMenu v-for="{ node, depth, expanded } in rows" :key="node.id" :items="() => menuItems(node)">
      <template #default="{ onContextMenu }">
        <div
          :data-tree-node="node.id"
          role="treeitem"
          :aria-level="depth + 1"
          :aria-expanded="node.children?.length ? expanded : undefined"
          :aria-selected="navigation.selectedId === node.id"
          :aria-busy="creationById.get(node.id)?.status === 'saving'"
          :tabindex="navigation.selectedId === node.id ? 0 : -1"
          class="resource-tree-row"
          :class="{
            selected: navigation.selectedId === node.id,
            compact: settings.navigation.compact,
            'drop-before': drag.targetId === node.id && drag.dropPosition === 'before',
            'drop-after': drag.targetId === node.id && drag.dropPosition === 'after',
            'drop-inside': drag.targetId === node.id && drag.dropPosition === 'inside',
          }"
          :style="{ paddingLeft: `${8 + depth * 16}px` }"
          @click="!drag.active && navigation.select(node)"
          @contextmenu="
            navigation.selectedId = node.id;
            onContextMenu($event);
          "
          @mousedown="canDrag && (node.type === 'resource' || node.type === 'resource-group') && startDrag($event, node.id, node.type, { autoScroll: true, scrollContainer: root })"
          @mousemove="(node.type === 'resource' || node.type === 'resource-group') && updateTarget($event, node.id, node.type)"
          @mouseleave="clearTarget(node.id)"
        >
          <button
            v-if="node.children?.length"
            type="button"
            class="flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            :aria-label="t(expanded ? 'tree.collapseNode' : 'tree.expandNode', { name: node.label })"
            :aria-expanded="expanded"
            @click.stop="navigation.toggle(node.id)"
          >
            <ChevronRight class="h-3 w-3 transition-transform" :class="{ 'rotate-90': expanded }" />
          </button>
          <span v-else class="w-3 shrink-0" />
          <component :is="node.providerRoot && node.providerIcon ? node.providerIcon : node.type === 'resource-group' || node.type === 'directory' ? (expanded ? FolderOpen : Folder) : (node.providerIcon ?? Box)" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <input
            v-if="navigation.naming?.id === node.id"
            :ref="nameInput"
            v-model="navigation.naming.name"
            :aria-label="t('common.name')"
            class="tree-name-input"
            :disabled="navigation.busy"
            @click.stop
            @mousedown.stop
            @keydown.enter.stop.prevent="commit"
            @keydown.escape.stop.prevent="navigation.naming = null"
            @blur="commit"
          />
          <span v-else class="min-w-0 flex-1 truncate" :title="node.label"
            >{{ node.type === "resource-group" ? displayGroupName(node.id, node.label) : node.label
            }}<small v-if="node.type === 'resource' && settings.navigation.showDescriptions" class="ml-2 text-muted-foreground">{{ workspace.catalog?.services.find((service) => service.id === node.id)?.description }}</small></span
          >
          <span v-if="creationById.get(node.id)" class="ml-auto flex shrink-0 items-center gap-1 text-[10px]" :class="creationById.get(node.id)?.status === 'failed' ? 'text-destructive' : 'text-muted-foreground'" role="status">
            <LoaderCircle v-if="creationById.get(node.id)?.status === 'saving'" class="h-3 w-3 animate-spin" />
            <span :title="creationById.get(node.id)?.error">{{ creationById.get(node.id)?.status === "saving" ? t("creation.saving") : t("creation.failed") }}</span>
            <template v-if="creationById.get(node.id)?.status === 'failed'">
              <button class="rounded p-1 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('creation.retry', { name: node.label })" @mousedown.stop @click.stop="creations.retry(node.id)"><RotateCcw class="h-3 w-3" /></button>
              <button class="rounded p-1 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('creation.discardNamed', { name: node.label })" @mousedown.stop @click.stop="creations.discard(node.id)"><X class="h-3 w-3" /></button>
            </template>
          </span>
          <span v-else-if="node.children?.length && !navigation.naming" class="ml-auto pr-2 text-[10px] text-muted-foreground">{{ node.children?.length ?? 0 }}</span>
        </div>
      </template>
    </CustomContextMenu>
    <p v-for="creation in creations.current.filter((item) => item.status === 'failed')" :key="`error-${creation.id}`" class="mx-2 my-1 break-words rounded border border-destructive/30 p-2 text-xs text-destructive" role="alert">
      {{ creation.name }}: {{ creation.error || t("creation.interrupted") }}
    </p>
    <p v-if="!rows.length" class="p-5 text-center text-xs text-muted-foreground">{{ searchActive ? t("tree.searchEmpty") : t("tree.initialEmpty") }}</p>
  </div>
</template>
