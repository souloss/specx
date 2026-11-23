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
    kind: "markdown" | "openapi";
    content: string;
    version: number;
    contentHash: string;
    updatedAt: string;
    etag: string;
  }>;
};

export type TagResponse = CatalogResponse["tags"][number];

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
export type RepositoryRefsResponse = { defaultBranch: string; branches: string[]; tags: string[] };
export type SpecificationRefKind = "branch" | "tag" | "commit";
export type SecretResponse = { id: string; name: string; keyVersion: number; createdAt: string; updatedAt: string };
export type SecretPageResponse = { items: SecretResponse[] };
export type IntegrationResponse = { id: string; provider: string; displayName: string; secretId: string | null; permissions: string[]; createdAt: string; updatedAt: string };
export type IntegrationPageResponse = { items: IntegrationResponse[] };
export type OpenApiEnvironmentVariable = { key: string; value: string | null; sensitive: boolean; enabled: boolean; hasValue: boolean };
export type OpenApiEnvironmentServer = { id: string; name: string; baseUrl: string; requestMode: "inherit" | "browser" | "agent" };
export type OpenApiEnvironmentInput = {
  name: string;
  baseUrl: string;
  requestMode?: OpenApiEnvironmentServer["requestMode"];
  servers?: OpenApiEnvironmentServer[];
  variables: Array<{ key: string; value?: string | null; sensitive: boolean; enabled: boolean }>;
};
export type OpenApiEnvironment = {
  id: string;
  serviceId: string;
  name: string;
  slug: string;
  baseUrl: string;
  requestMode?: "inherit" | "browser" | "agent";
  servers?: OpenApiEnvironmentServer[];
  variables: OpenApiEnvironmentVariable[];
  createdAt: string;
  updatedAt: string;
};
export type OpenApiRuntimeEnvironment = {
  environmentId: string;
  baseUrl: string;
  requestMode?: "inherit" | "browser" | "agent";
  servers?: OpenApiEnvironmentServer[];
  variables: Array<{ key: string; value: string }>;
};
export type OpenApiSettings = {
  defaultEnvironmentId: string | null;
  defaultServerUrl: string;
  showOperationId: boolean;
  schemaDisplay: "summary" | "full";
  mockEnabled: boolean;
  updatedAt: string;
};
export type OpenApiProviderResponse = {
  serviceId: string;
  settings: OpenApiSettings;
  environments: OpenApiEnvironment[];
  specificationCount: number;
  operationCount: number;
  tagCount: number;
};
export type OpenApiMockResponse = { enabled: boolean; token: string | null };

export type AgentRequest = { method: string; url: string; headers: Array<[string, string]>; disabledHeaders?: string[]; bodyBase64: string; timeoutMs: number; followRedirects: boolean };
type AgentTls = { protocol: string | null; cipher: string | null; certificateCommonName: string | null; issuerCommonName: string | null; expiresAt: string | null };
export type AgentResponse = {
  status: number;
  statusText: string;
  headers: Array<[string, string]>;
  bodyBase64: string;
  truncated: boolean;
  url: string;
  redirects: number;
  timing: { dnsMs: number; connectMs: number; tlsMs: number; firstByteMs: number; downloadMs: number; totalMs: number };
  network: { localIp: string | null; localPort: number; remoteIp: string | null; remotePort: number; protocol: string; tls: AgentTls | null };
  requestHeaders: Array<[string, string]>;
  requestMethod: string;
  requestBodyBase64: string;
  requestHeaderBytes: number;
  requestBodyBytes: number;
  responseHeaderBytes: number;
  responseBodyBytes: number;
};
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
  snapshotId: string;
  manifestHash: string;
  resolvedCommit: string;
  contentHash: string;
  changed: boolean;
  syncedAt: string;
  providerVersion: string;
};
export type RepositoryFileResponse = { path: string; byteLength: number; contentHash: string };
export type RepositoryDiscoveryResponse = {
  snapshotId: string;
  resolvedCommit: string;
  manifestHash: string;
  providerVersion: string;
  files: RepositoryFileResponse[];
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

export type OpenApiPublication = {
  id: string;
  version: number;
  title: string;
  description: string;
  theme: "light" | "dark";
  showDownload: boolean;
  allowTryIt: boolean;
  createdAt: string;
};
export type OpenApiPublicationPage = { items: OpenApiPublication[]; nextOffset: number | null };
export type OpenApiPublicationPreview = { content: string; digest: string };
export type PublicOpenApiDocument = { publication: OpenApiPublication; content: string };

/** Explicit, portable public values. Never contains secret references. */
export type PublicOpenApiEnvironment = {
  name: string;
  baseUrl: string;
  variables: Array<{ key: string; value: string }>;
};
