export type WorkspaceResponse = {
  id: string;
  tenantId: string;
  name: string;
  slug: string;
  role: string;
  createdAt: string;
};

export type WorkspaceListResponse = { items: WorkspaceResponse[] };
export type AuthResponse = { authenticated: boolean; setup_required: boolean };

export type CatalogResponse = {
  groups: Array<{ id: string; parentId: string | null; name: string; position: number }>;
  tags: Array<{ id: string; name: string; color: string }>;
  services: Array<{
    id: string;
    groupId: string | null;
    name: string;
    slug: string;
    description: string;
    tagIds: string[];
    updatedAt: string;
    archived: boolean;
    etag: string;
  }>;
  specifications: Array<{
    id: string;
    serviceId: string;
    title: string;
    kind: "markdown";
    content: string;
    version: number;
    contentHash: string;
    updatedAt: string;
    etag: string;
  }>;
};

export type SpecificationResponse = CatalogResponse["specifications"][number];
export type SpecificationRevisionResponse = {
  specificationId: string;
  version: number;
  content: string;
  contentHash: string;
  createdBy: string;
  createdAt: string;
};
export type SpecificationRevisionPageResponse = {
  items: SpecificationRevisionResponse[];
  total: number;
  limit: number;
  offset: number;
  nextOffset?: number;
};

export type DesktopApiRequest = {
  method: string;
  path: string;
  headers: Record<string, string>;
  body?: string;
};

export type DesktopApiResponse = {
  status: number;
  headers: Record<string, string>;
  body: unknown;
};
