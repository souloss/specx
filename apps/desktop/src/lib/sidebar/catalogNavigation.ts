import type { Catalog } from "@/types/domain";
import type { ResourceTreeNode, SidebarLayout, SidebarOrderEntry } from "@/types/navigation";
import { UNGROUPED_GROUP_ID } from "@/lib/core";

/** The catalog owns hierarchy; browser storage owns only display order and expansion.
 * A stale local layout must never move a shared service back to an old group. */
export function catalogSidebarLayout(catalog: Catalog, saved?: SidebarLayout | null): SidebarLayout {
  const collapsed = new Map(saved?.groups.map((group) => [group.id, group.collapsed]));
  const orderIndex = new Map<string, number>();
  function index(entries: SidebarOrderEntry[]) {
    entries.forEach((entry, position) => {
      orderIndex.set(entry.id, position);
      if (entry.type === "group") index(entry.children ?? []);
    });
  }
  index(saved?.order ?? []);
  const groups = catalog.groups.filter((group) => group.id !== UNGROUPED_GROUP_ID);
  const byParent = new Map<string | null, SidebarOrderEntry[]>();
  const append = (parent: string | null, entry: SidebarOrderEntry) => {
    if (!byParent.has(parent)) byParent.set(parent, []);
    byParent.get(parent)!.push(entry);
  };
  const groupIds = new Set(groups.map((group) => group.id));
  for (const group of [...groups].sort((a, b) => (a.position ?? 0) - (b.position ?? 0))) append(group.parentId && groupIds.has(group.parentId) ? group.parentId : null, { type: "group", id: group.id, children: [] });
  for (const service of catalog.services) append(groupIds.has(service.groupId) ? service.groupId : null, { type: "resource", id: service.id });
  const visited = new Set<string>();
  function children(parent: string | null): SidebarOrderEntry[] {
    const entries = (byParent.get(parent) ?? []).filter((entry) => !visited.has(entry.id));
    // Group ordering is shared server state; manual resource ordering is a local preference.
    const resourceEntries = entries.filter((entry) => entry.type === "resource").sort((a, b) => (orderIndex.get(a.id) ?? Infinity) - (orderIndex.get(b.id) ?? Infinity));
    let resourceIndex = 0;
    return entries.map((entry) => {
      visited.add(entry.id);
      return entry.type === "group" ? { ...entry, children: children(entry.id) } : resourceEntries[resourceIndex++];
    });
  }
  return { groups: groups.map((group) => ({ id: group.id, name: group.name, collapsed: collapsed.get(group.id) ?? false })), order: children(null) };
}

export function layoutTree(layout: SidebarLayout, catalog: Catalog): ResourceTreeNode[] {
  const names = new Map([...catalog.groups, ...catalog.services].map((item) => [item.id, item.name]));
  function visit(entries: SidebarOrderEntry[], parentId: string | null): ResourceTreeNode[] {
    return entries.map((entry) => ({ id: entry.id, label: names.get(entry.id) ?? "", parentId, type: entry.type === "group" ? "resource-group" : "resource", children: entry.type === "group" ? visit(entry.children ?? [], entry.id) : undefined }));
  }
  return visit(layout.order, null);
}

/** Keep matching ancestors and matching groups' descendants visible during search. */
export function filterResourceTree(nodes: ResourceTreeNode[], query: string, catalog: Catalog, tagId = ""): ResourceTreeNode[] {
  const needle = query.trim().toLocaleLowerCase();
  const services = new Map(catalog.services.map((service) => [service.id, service]));
  function visit(items: ResourceTreeNode[], ancestorMatches = false): ResourceTreeNode[] {
    return items.flatMap((node) => {
      const service = services.get(node.id);
      const textMatch = !needle || ancestorMatches || [node.label, service?.slug, service?.description].some((text) => text?.toLocaleLowerCase().includes(needle));
      if (node.type === "resource") return textMatch && (!tagId || service?.tagIds.includes(tagId)) ? [node] : [];
      const children = visit(node.children ?? [], textMatch && !!needle);
      return children.length || (textMatch && !tagId) ? [{ ...node, children }] : [];
    });
  }
  return visit(nodes);
}

export function layoutPlacements(layout: SidebarLayout) {
  const groups: Array<{ id: string; parentId: string | null; position: number }> = [];
  const resources = new Map<string, string | null>();
  function visit(entries: SidebarOrderEntry[], parentId: string | null) {
    entries.forEach((entry, position) => {
      if (entry.type === "group") {
        groups.push({ id: entry.id, parentId, position });
        visit(entry.children ?? [], entry.id);
      } else resources.set(entry.id, parentId);
    });
  }
  visit(layout.order, null);
  return { groups, resources };
}
