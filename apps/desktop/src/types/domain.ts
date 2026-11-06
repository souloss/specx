export type Group = { id: string; name: string; parentId: string | null; position?: number };
export type Tag = { id: string; name: string; color: string };
export type SpecificationKind = "markdown";
export type Specification = {
  id: string;
  serviceId: string;
  title: string;
  kind: SpecificationKind;
  content: string;
  version: number;
  contentHash: string;
  updatedAt: string;
  etag: string;
};
export type SpecificationRevision = {
  specificationId: string;
  version: number;
  content: string;
  contentHash: string;
  createdBy: string;
  createdAt: string;
};
export type Service = {
  id: string;
  name: string;
  slug: string;
  description: string;
  groupId: string;
  tagIds: string[];
  updatedAt: string;
  createdAt: string;
  etag?: string;
  archived?: boolean;
};
export type CodeSource = import("./api").CodeSourceResponse;
export type ServiceCodeBinding = import("./api").ServiceCodeBindingResponse;
export type Workspace = { id: string; name: string; tenantName: string };
export type Catalog = {
  workspace: Workspace;
  groups: Group[];
  tags: Tag[];
  services: Service[];
  specifications: Specification[];
};

export type PanelTab = {
  id: string;
  kind: "overview" | "specification";
  serviceId: string;
  specificationId?: string;
  title: string;
};
