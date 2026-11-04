/* Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. */
import type { ResourceConfig, ResourceTreeNode } from "@/types/navigation";

export type ResourceListSortMode = "manual" | "asc" | "desc";

const resourceNameCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

function sortResourceSiblingsForDisplay(nodes: readonly ResourceTreeNode[], mode: Exclude<ResourceListSortMode, "manual">): ResourceTreeNode[] {
  const sortedResources = nodes
    .map((node, index) => ({ node, index }))
    .filter(({ node }) => node.type === "resource")
    .sort((left, right) => {
      const compared = resourceNameCollator.compare(left.node.label, right.node.label);
      const directional = mode === "asc" ? compared : -compared;
      return directional || left.index - right.index;
    })
    .map(({ node }) => node);

  let nextResourceIndex = 0;
  const displayNodes = nodes.map((node) => {
    if (node.type === "resource") {
      return sortedResources[nextResourceIndex++]!;
    }

    if (node.type !== "resource-group" || !node.children) return node;
    const children = sortResourceSiblingsForDisplay(node.children, mode);
    return children === node.children ? node : { ...node, children };
  });

  return displayNodes.every((node, index) => node === nodes[index]) ? (nodes as ResourceTreeNode[]) : displayNodes;
}

/**
 * Produces a display-only ordering of saved resources. Group positions and
 * the persisted manual layout remain untouched, so switching back to manual
 * order restores drag-and-drop placement without a data migration.
 */
export function sortResourceListForDisplay(nodes: readonly ResourceTreeNode[], mode: ResourceListSortMode): ResourceTreeNode[] {
  if (mode === "manual") return nodes as ResourceTreeNode[];
  return sortResourceSiblingsForDisplay(nodes, mode);
}

/**
 * Projects resource configs into the order shown by the sidebar. Resources
 * missing from the current tree are appended in store order so a stale layout
 * or newly added resource cannot make them unavailable to other surfaces.
 */
export function orderResourcesForSidebarDisplay<T extends Pick<ResourceConfig, "id">>(resources: readonly T[], nodes: readonly ResourceTreeNode[], mode: ResourceListSortMode): T[] {
  const resourcesById = new Map<string, T>();
  for (const resource of resources) {
    if (!resourcesById.has(resource.id)) resourcesById.set(resource.id, resource);
  }

  const ordered: T[] = [];
  const seen = new Set<string>();
  const appendResource = (id: string) => {
    const resource = resourcesById.get(id);
    if (!resource || seen.has(id)) return;
    seen.add(id);
    ordered.push(resource);
  };
  const visit = (treeNodes: readonly ResourceTreeNode[]) => {
    for (const node of treeNodes) {
      if (node.type === "resource") appendResource(node.resourceId ?? node.id);
      else if (node.type === "resource-group" && node.children) visit(node.children);
    }
  };

  visit(sortResourceListForDisplay(nodes, mode));
  for (const resource of resources) appendResource(resource.id);
  return ordered;
}
