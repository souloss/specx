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

export type CodeSourceType = "git" | "local_filesystem";
export type CodeExecutionScope = "server" | "desktop_host";
export type CodeBindingRole = "primary" | "additional";
export type CodeEntityKind = "repository_root" | "directory" | "file" | "pattern";
export type CodeRevisionKind = "default_branch" | "branch" | "tag" | "commit" | "working_tree";
export type CodeSourceResponse = {
  id: string;
  tenantId: string;
  workspaceId: string;
  sourceType: CodeSourceType;
  provider: string;
  displayName: string;
  canonicalKey: string;
  rootLocator: string;
  executionScope: CodeExecutionScope;
  createdAt: string;
  updatedAt: string;
  archived: boolean;
  etag: string;
};
export type CodeSourcePageResponse = { items: CodeSourceResponse[] };
export type ServiceCodeBindingResponse = {
  id: string;
  tenantId: string;
  workspaceId: string;
  serviceId: string;
  sourceId: string;
  role: CodeBindingRole;
  entityKind: CodeEntityKind;
  path: string;
  revisionKind: CodeRevisionKind;
  revision: string | null;
  createdAt: string;
  updatedAt: string;
  etag: string;
};
export type ServiceCodeBindingPageResponse = { items: ServiceCodeBindingResponse[] };
export type RepositorySummaryResponse = { id: string; provider: string; displayName: string; canonicalUrl: string };
export type ServiceRepositoryResponse = { id: string; repository: RepositorySummaryResponse; createdAt: string; updatedAt: string; etag: string };
export type SpecificationRefKind = "branch" | "tag" | "commit";
export type SpecificationCodeBindingResponse = {
  id: string;
  specificationId: string;
  repository: RepositorySummaryResponse;
  path: string;
  refKind: SpecificationRefKind;
  refName: string;
  createdAt: string;
  updatedAt: string;
  etag: string;
};
export type SpecificationSyncResponse = {
  specification: SpecificationResponse;
  resolvedCommit: string;
  contentHash: string;
  changed: boolean;
  syncedAt: string;
  providerVersion: string;
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
