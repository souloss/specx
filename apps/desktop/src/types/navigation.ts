/** The data carried by the database-neutral sidebar layout, adapted to SpecX resources. */
export type ResourceGroup = { id: string; name: string; collapsed: boolean };
export type SidebarOrderEntry = { type: "group"; id: string; children?: SidebarOrderEntry[]; resourceIds?: string[] } | { type: "resource"; id: string };
export type SidebarLayout = { groups: ResourceGroup[]; order: SidebarOrderEntry[] };

export type ResourceTreeNode = {
  id: string;
  label: string;
  type: "resource" | "resource-group";
  parentId?: string | null;
  resourceId?: string;
  children?: ResourceTreeNode[];
};
export type ResourceConfig = { id: string; name: string };
