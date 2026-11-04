import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { useWorkspaceStore } from "./workspace";
import { useCreateQueueStore } from "./createQueueStore";
import { useSettingsStore } from "./settingsStore";
import { catalogSidebarLayout, filterResourceTree, layoutPlacements, layoutTree } from "@/lib/sidebar/catalogNavigation";
import { collapseAllGroups, expandGroups, reorderEntry, toggleGroupCollapsed, type DropPosition } from "@/lib/sidebar/sidebarLayout";
import { sortResourceListForDisplay } from "@/lib/sidebar/resourceListSort";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { specxApi } from "@/lib/backend/client";
import { createId, UNGROUPED_GROUP_ID } from "@/lib/core";
import type { ResourceTreeNode, SidebarLayout } from "@/types/navigation";

export type NamingDraft = { id: string; kind: "group" | "resource"; name: string; parentId: string | null; creating: boolean };

/** Navigation adapts the layout engine to catalog commands. UI components
 * receive tree nodes and never make HTTP or Tauri calls themselves. */
export const useNavigationStore = defineStore("navigation", () => {
  const workspace = useWorkspaceStore();
  const settings = useSettingsStore();
  const creations = useCreateQueueStore();
  const layout = ref<SidebarLayout>({ groups: [], order: [] });
  const query = ref("");
  const tagId = ref("");
  const selectedId = ref("");
  const naming = ref<NamingDraft | null>(null);
  const busy = ref(false);
  const revealRevision = ref(0);
  const storageKey = computed(() => `specx-sidebar-layout:${workspace.catalog?.workspace.id}`);

  watch(
    () => workspace.catalog,
    (catalog, previous) => {
      if (!catalog) return;
      let saved: SidebarLayout | null = null;
      try {
        saved = JSON.parse(safeLocalStorageGet(storageKey.value) ?? "null");
      } catch {
        /* Rebuild stale storage. */
      }
      if (!saved || !Array.isArray(saved.groups) || !Array.isArray(saved.order)) saved = null;
      try {
        layout.value = catalogSidebarLayout(catalog, saved);
      } catch {
        layout.value = catalogSidebarLayout(catalog);
      }
      if (previous?.workspace.id === catalog.workspace.id && !previous.services.some((item) => item.id === selectedId.value) && catalog.services.some((item) => item.id === selectedId.value)) workspace.selectService(selectedId.value);
      if (!catalog.groups.some((item) => item.id === selectedId.value) && !catalog.services.some((item) => item.id === selectedId.value) && !creations.current.some((item) => item.id === selectedId.value)) selectedId.value = "";
    },
    { immediate: true },
  );
  watch(
    layout,
    (value) => {
      if (workspace.catalog) safeLocalStorageSet(storageKey.value, JSON.stringify(value));
    },
    { deep: true },
  );
  watch(
    () => workspace.activeServiceId,
    (id) => {
      if (id) selectedId.value = id;
    },
  );

  const tree = computed(() => {
    if (!workspace.catalog) return [];
    let nodes = filterResourceTree(layoutTree(layout.value, workspace.catalog), query.value, workspace.catalog, tagId.value);
    nodes = sortResourceListForDisplay(nodes, settings.navigation.sortMode);
    const entries = [...creations.current, ...(naming.value?.creating ? [naming.value] : [])];
    for (const draft of entries) {
      if (workspace.catalog.groups.some((item) => item.id === draft.id) || workspace.catalog.services.some((item) => item.id === draft.id)) continue;
      const node: ResourceTreeNode = { id: draft.id, type: draft.kind === "group" ? "resource-group" : "resource", label: draft.name, parentId: draft.parentId, children: [] };
      const insert = (items: ResourceTreeNode[]): boolean => {
        for (const item of items) {
          if (item.id === draft.parentId) {
            (item.children ??= []).push(node);
            return true;
          }
          if (insert(item.children ?? [])) return true;
        }
        return false;
      };
      if (!draft.parentId || !insert(nodes)) nodes = [...nodes, node];
    }
    return nodes;
  });
  const expandedIds = computed(() => new Set(layout.value.groups.filter((group) => !group.collapsed).map((group) => group.id)));
  function toggle(id: string) {
    layout.value = toggleGroupCollapsed(layout.value, id);
  }
  function collapseAll() {
    layout.value = collapseAllGroups(layout.value);
  }
  function expandAll() {
    layout.value = expandGroups(
      layout.value,
      layout.value.groups.map((group) => group.id),
    );
  }
  function select(node: ResourceTreeNode) {
    selectedId.value = node.id;
    if (creations.current.some((item) => item.id === node.id)) return;
    if (node.type === "resource") {
      settings.showSettingsPage = false;
      workspace.selectService(node.id);
    } else toggle(node.id);
  }
  function beginCreate(kind: NamingDraft["kind"], parentId: string | null = null) {
    if (busy.value || naming.value || creations.current.some((item) => item.id === parentId)) return;
    query.value = "";
    tagId.value = "";
    if (parentId) layout.value = expandGroups(layout.value, [parentId]);
    naming.value = { id: createId(kind === "group" ? "grp" : "svc"), kind, name: "", parentId: parentId === UNGROUPED_GROUP_ID ? null : parentId, creating: true };
  }
  function beginRename(node: ResourceTreeNode) {
    if (!busy.value && !naming.value && !creations.current.some((item) => item.id === node.id)) naming.value = { id: node.id, kind: node.type === "resource" ? "resource" : "group", name: node.label, parentId: node.parentId ?? null, creating: false };
  }
  async function commitName() {
    const draft = naming.value;
    const catalog = workspace.catalog;
    if (!draft || busy.value || !catalog) return;
    const name = draft.name.trim();
    if (!name) {
      naming.value = null;
      return;
    }
    if (draft.creating) {
      naming.value = null;
      selectedId.value = draft.id;
      creations.enqueue({ id: draft.id, workspaceId: catalog.workspace.id, kind: draft.kind, name, parentId: draft.parentId });
      return;
    }
    busy.value = true;
    try {
      if (draft.kind === "group") {
        const group = catalog.groups.find((item) => item.id === draft.id);
        if (group) await specxApi.updateGroup(catalog.workspace.id, { ...group, name });
      } else {
        const service = catalog.services.find((item) => item.id === draft.id);
        if (service) await specxApi.updateService(catalog.workspace.id, { ...service, name });
      }
      naming.value = null;
      await workspace.refresh();
      if (draft.kind === "resource") workspace.selectService(draft.id);
    } finally {
      busy.value = false;
    }
  }
  async function remove(node: ResourceTreeNode) {
    const catalog = workspace.catalog;
    if (!catalog || busy.value || creations.current.some((item) => item.id === node.id || item.parentId === node.id)) return;
    busy.value = true;
    try {
      if (node.type === "resource-group") await specxApi.deleteGroup(catalog.workspace.id, node.id);
      else {
        const service = catalog.services.find((item) => item.id === node.id);
        if (service) await specxApi.deleteService(catalog.workspace.id, service);
      }
      await workspace.refresh();
    } finally {
      busy.value = false;
    }
  }
  async function drop(draggedId: string, targetId: string, position: DropPosition) {
    const catalog = workspace.catalog;
    if (!catalog || busy.value || naming.value || creations.current.length || query.value || tagId.value || settings.navigation.sortMode !== "manual") return;
    const next = reorderEntry(layout.value, draggedId, targetId, position);
    if (next === layout.value) return;
    const placements = layoutPlacements(next);
    busy.value = true;
    try {
      const movedService = catalog.services.find((service) => service.id === draggedId);
      if (movedService) {
        const parent = placements.resources.get(draggedId) ?? UNGROUPED_GROUP_ID;
        if (parent !== movedService.groupId) await specxApi.updateService(catalog.workspace.id, { ...movedService, groupId: parent });
      } else await specxApi.reorderGroups(catalog.workspace.id, placements.groups);
      layout.value = next;
      safeLocalStorageSet(storageKey.value, JSON.stringify(next));
      await workspace.refresh();
    } catch (error) {
      await workspace.refresh();
      throw error;
    } finally {
      busy.value = false;
    }
  }
  function locateActive() {
    if (!workspace.catalog || !workspace.activeServiceId) return;
    query.value = "";
    tagId.value = "";
    selectedId.value = workspace.activeServiceId;
    const service = workspace.catalog.services.find((item) => item.id === selectedId.value);
    const ancestors: string[] = [];
    let id = service?.groupId;
    while (id && !ancestors.includes(id)) {
      ancestors.push(id);
      id = workspace.catalog.groups.find((group) => group.id === id)?.parentId ?? undefined;
    }
    layout.value = expandGroups(layout.value, ancestors);
    revealRevision.value++;
  }
  async function moveResource(id: string, parentId: string | null) {
    const catalog = workspace.catalog;
    const service = catalog?.services.find((item) => item.id === id);
    if (!catalog || !service || busy.value || creations.current.some((item) => item.id === parentId)) return;
    busy.value = true;
    try {
      await specxApi.updateService(catalog.workspace.id, { ...service, groupId: parentId ?? UNGROUPED_GROUP_ID });
      await workspace.refresh();
    } finally {
      busy.value = false;
    }
  }
  return { layout, tree, expandedIds, query, tagId, selectedId, naming, busy, revealRevision, toggle, select, collapseAll, expandAll, beginCreate, beginRename, commitName, remove, drop, locateActive, moveResource };
});
