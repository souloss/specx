import { uuid } from "../common/utils";
import type { Catalog, Group, Service, Specification } from "../../types/domain";
import type {
  AuthResponse,
  CatalogResponse,
  CodeSourcePageResponse,
  CodeSourceResponse,
  ServiceCodeBindingPageResponse,
  ServiceCodeBindingResponse,
  ServiceRepositoryResponse,
  SpecificationCodeBindingResponse,
  SpecificationRefKind,
  SpecificationResponse,
  SpecificationSyncResponse,
  RepositoryDiscoveryResponse,
  RepositoryRefsResponse,
  SecretPageResponse,
  SecretResponse,
  IntegrationPageResponse,
  IntegrationResponse,
  SpecificationRevisionPageResponse,
  SpecificationRevisionResponse,
  WorkspaceListResponse,
  WorkspaceResponse,
} from "../../types/api";
import { UNGROUPED_GROUP_ID } from "../core";
import { getBackendTransport } from "./api";
import { ApiError } from "./types";
import { isTauriRuntime } from "./runtime";
import { translate } from "@/i18n";
import { trackWrite } from "./writeActivity";

async function request<T>(path: string, options: { method?: string; headers?: Record<string, string>; body?: unknown } = {}, workspaceId?: string): Promise<T> {
  const headers = { ...options.headers };
  if (workspaceId) headers["x-specx-workspace-id"] = workspaceId;
  const send = async () => (await getBackendTransport()).request<T>(path, { ...options, headers });
  return options.method && !["GET", "HEAD"].includes(options.method) ? trackWrite(send) : send();
}

function idempotencyKey(prefix: string) {
  return `${prefix}-${uuid()}`;
}

function toCatalog(workspace: WorkspaceResponse, catalog: CatalogResponse): Catalog {
  const groups: Group[] = catalog.groups.map(({ id, name, parentId, position }) => ({ id, name, parentId, position }));
  if (catalog.services.some((service) => service.groupId === null)) {
    groups.push({ id: UNGROUPED_GROUP_ID, name: translate("common.ungrouped"), parentId: null });
  }
  const services = catalog.services.map(toService);
  const specifications: Specification[] = catalog.specifications.map((specification) => ({
    ...specification,
    kind: "markdown",
  }));
  return {
    workspace: { id: workspace.id, name: workspace.name, tenantName: workspace.tenantId },
    groups,
    tags: catalog.tags,
    services,
    specifications,
  };
}

export function toService(service: CatalogResponse["services"][number]): Service {
  return { ...service, groupId: service.groupId ?? UNGROUPED_GROUP_ID, createdAt: service.updatedAt };
}

export const specxApi = {
  isDesktop: () => isTauriRuntime(),
  migrationRetry: async () => {
    if (!isTauriRuntime()) return { ready: true };
    const { retryDesktopMigration } = await import("./tauri");
    return retryDesktopMigration();
  },
  authCheck: () => request<AuthResponse>("/api/auth/check"),
  login: (password: string) => request<AuthResponse>("/api/auth/login", { method: "POST", body: { password } }),
  logout: () => request<void>("/api/auth/logout", { method: "POST" }),
  listWorkspaces: () => request<WorkspaceListResponse>("/api/specx/workspaces"),

  async loadCatalog(workspaceId?: string, availableWorkspaces?: WorkspaceResponse[]): Promise<Catalog> {
    const workspaces = availableWorkspaces ?? (await specxApi.listWorkspaces()).items;
    const workspace = workspaceId ? workspaces.find((item) => item.id === workspaceId) : workspaces[0];
    if (!workspace) throw new ApiError(translate("apiError.forbidden"), 403);
    const catalog = await request<CatalogResponse>("/api/specx/catalog", {}, workspace.id);
    return toCatalog(workspace, catalog);
  },

  createGroup(workspaceId: string, input: { id: string; name: string; parentId: string | null; position?: number }, key = idempotencyKey("group")) {
    return request<CatalogResponse["groups"][number]>(
      "/api/specx/groups",
      {
        method: "POST",
        headers: { "idempotency-key": key },
        body: input,
      },
      workspaceId,
    );
  },

  createService(
    workspaceId: string,
    input: {
      id: string;
      groupId: string | null;
      name: string;
      slug: string;
      description: string;
      tagIds: string[];
    },
    key = idempotencyKey("service"),
  ) {
    return request<CatalogResponse["services"][number]>(
      "/api/specx/services",
      {
        method: "POST",
        headers: { "idempotency-key": key },
        body: input,
      },
      workspaceId,
    );
  },

  updateGroup(workspaceId: string, group: Group) {
    return request<unknown>(`/api/specx/groups/${encodeURIComponent(group.id)}`, { method: "PUT", body: { name: group.name, parentId: group.parentId, position: group.position } }, workspaceId);
  },
  reorderGroups(workspaceId: string, items: Array<{ id: string; parentId: string | null; position: number }>) {
    return request<unknown>("/api/specx/groups/order", { method: "PUT", body: { items } }, workspaceId);
  },
  deleteGroup(workspaceId: string, id: string) {
    return request<void>(`/api/specx/groups/${encodeURIComponent(id)}`, { method: "DELETE" }, workspaceId);
  },
  updateService(workspaceId: string, service: Service) {
    return request<unknown>(
      `/api/specx/services/${encodeURIComponent(service.id)}`,
      {
        method: "PUT",
        headers: { "if-match": service.etag ?? "" },
        body: { name: service.name, slug: service.slug, description: service.description, groupId: service.groupId === UNGROUPED_GROUP_ID ? null : service.groupId, tagIds: service.tagIds },
      },
      workspaceId,
    );
  },
  deleteService(workspaceId: string, service: Service) {
    return request<void>(`/api/specx/services/${encodeURIComponent(service.id)}`, { method: "DELETE", headers: { "if-match": service.etag ?? "" } }, workspaceId);
  },
  listCodeSources(workspaceId: string) {
    return request<CodeSourcePageResponse>("/api/specx/code-sources", {}, workspaceId);
  },
  createCodeSource(workspaceId: string, input: Pick<CodeSourceResponse, "id" | "sourceType" | "provider" | "displayName" | "canonicalKey" | "rootLocator" | "executionScope">, key = idempotencyKey("code-source")) {
    return request<CodeSourceResponse>("/api/specx/code-sources", { method: "POST", headers: { "idempotency-key": key }, body: input }, workspaceId);
  },
  updateCodeSource(workspaceId: string, source: CodeSourceResponse) {
    return request<CodeSourceResponse>(
      `/api/specx/code-sources/${encodeURIComponent(source.id)}`,
      { method: "PUT", headers: { "if-match": source.etag }, body: { sourceType: source.sourceType, provider: source.provider, displayName: source.displayName, canonicalKey: source.canonicalKey, rootLocator: source.rootLocator, executionScope: source.executionScope } },
      workspaceId,
    );
  },
  deleteCodeSource(workspaceId: string, source: CodeSourceResponse) {
    return request<void>(`/api/specx/code-sources/${encodeURIComponent(source.id)}`, { method: "DELETE", headers: { "if-match": source.etag } }, workspaceId);
  },
  listCodeBindings(workspaceId: string, serviceId: string) {
    return request<ServiceCodeBindingPageResponse>(`/api/specx/services/${encodeURIComponent(serviceId)}/code-bindings`, {}, workspaceId);
  },
  createCodeBinding(workspaceId: string, serviceId: string, input: Pick<ServiceCodeBindingResponse, "id" | "sourceId" | "role" | "entityKind" | "path" | "revisionKind" | "revision">, key = idempotencyKey("code-binding")) {
    return request<ServiceCodeBindingResponse>(`/api/specx/services/${encodeURIComponent(serviceId)}/code-bindings`, { method: "POST", headers: { "idempotency-key": key }, body: input }, workspaceId);
  },
  updateCodeBinding(workspaceId: string, binding: ServiceCodeBindingResponse) {
    return request<ServiceCodeBindingResponse>(
      `/api/specx/code-bindings/${encodeURIComponent(binding.id)}`,
      { method: "PUT", headers: { "if-match": binding.etag }, body: { role: binding.role, entityKind: binding.entityKind, path: binding.path, revisionKind: binding.revisionKind, revision: binding.revision } },
      workspaceId,
    );
  },
  deleteCodeBinding(workspaceId: string, binding: ServiceCodeBindingResponse) {
    return request<void>(`/api/specx/code-bindings/${encodeURIComponent(binding.id)}`, { method: "DELETE", headers: { "if-match": binding.etag } }, workspaceId);
  },
  getServiceRepository(workspaceId: string, serviceId: string) {
    return request<ServiceRepositoryResponse>(`/api/specx/services/${encodeURIComponent(serviceId)}/repository`, {}, workspaceId);
  },
  linkServiceRepository(workspaceId: string, serviceId: string, repositoryUrl: string, key = idempotencyKey("service-repository")) {
    return request<ServiceRepositoryResponse>(`/api/specx/services/${encodeURIComponent(serviceId)}/repository`, { method: "POST", headers: { "idempotency-key": key }, body: { repositoryUrl } }, workspaceId);
  },
  unlinkServiceRepository(workspaceId: string, serviceId: string, etag: string) {
    return request<void>(`/api/specx/services/${encodeURIComponent(serviceId)}/repository`, { method: "DELETE", headers: { "if-match": etag } }, workspaceId);
  },
  getSpecificationCodeBinding(workspaceId: string, specificationId: string) {
    return request<SpecificationCodeBindingResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/code-binding`, {}, workspaceId);
  },
  setSpecificationCodeBinding(workspaceId: string, specificationId: string, refKind: SpecificationRefKind, refName: string, path: string, etag?: string) {
    return request<SpecificationCodeBindingResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/code-binding`, { method: "PUT", headers: etag ? { "if-match": etag } : {}, body: { path, refKind, refName } }, workspaceId);
  },
  deleteSpecificationCodeBinding(workspaceId: string, specificationId: string, etag: string) {
    return request<void>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/code-binding`, { method: "DELETE", headers: { "if-match": etag } }, workspaceId);
  },
  syncSpecification(workspaceId: string, specificationId: string) {
    return request<SpecificationSyncResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/sync`, { method: "POST" }, workspaceId);
  },
  getSpecificationSyncStatus(workspaceId: string, specificationId: string) {
    return request<SpecificationSyncResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/sync-status`, {}, workspaceId);
  },
  discoverRepositoryFiles(workspaceId: string, repositoryId: string, refKind: SpecificationRefKind, refName: string) {
    const query = new URLSearchParams({ refKind, refName });
    return request<RepositoryDiscoveryResponse>(`/api/specx/repositories/${encodeURIComponent(repositoryId)}/files?${query}`, {}, workspaceId);
  },
  discoverRepositoryRefs(workspaceId: string, repositoryId: string) {
    return request<RepositoryRefsResponse>(`/api/specx/repositories/${encodeURIComponent(repositoryId)}/refs`, {}, workspaceId);
  },
  listSecrets(workspaceId: string) {
    return request<SecretPageResponse>("/api/specx/secrets", {}, workspaceId);
  },
  createSecret(workspaceId: string, input: { id: string; name: string; value: string }) {
    return request<SecretResponse>("/api/specx/secrets", { method: "POST", body: input }, workspaceId);
  },
  updateSecret(workspaceId: string, secretId: string, value: string) {
    return request<SecretResponse>(`/api/specx/secrets/${encodeURIComponent(secretId)}`, { method: "PUT", body: { value } }, workspaceId);
  },
  listIntegrations(workspaceId: string) {
    return request<IntegrationPageResponse>("/api/specx/integrations", {}, workspaceId);
  },
  createIntegration(workspaceId: string, input: { id: string; provider: string; displayName: string; secretId: string | null; permissions: string[] }) {
    return request<IntegrationResponse>("/api/specx/integrations", { method: "POST", body: input }, workspaceId);
  },
  createTag(workspaceId: string, tag: { id: string; name: string; color: string }) {
    return request<unknown>("/api/specx/tags", { method: "POST", headers: { "idempotency-key": idempotencyKey("tag") }, body: tag }, workspaceId);
  },

  createSpecification(
    workspaceId: string,
    input: {
      id: string;
      serviceId: string;
      title: string;
      content: string;
      source?: { path: string; refKind: SpecificationRefKind; refName: string };
    },
  ) {
    return request<SpecificationResponse>(
      "/api/specx/specifications",
      {
        method: "POST",
        headers: { "idempotency-key": idempotencyKey("specification") },
        body: input,
      },
      workspaceId,
    );
  },
  updateSpecification(workspaceId: string, specificationId: string, input: { title: string; content: string }, etag: string) {
    return request<SpecificationResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}`, { method: "PUT", headers: { "if-match": etag }, body: input }, workspaceId);
  },
  deleteSpecification(workspaceId: string, specificationId: string, etag: string) {
    return request<void>(`/api/specx/specifications/${encodeURIComponent(specificationId)}`, { method: "DELETE", headers: { "if-match": etag } }, workspaceId);
  },
  listSpecificationRevisions(workspaceId: string, specificationId: string, limit = 50, offset = 0) {
    return request<SpecificationRevisionPageResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/revisions?limit=${limit}&offset=${offset}`, {}, workspaceId);
  },
  getSpecificationRevision(workspaceId: string, specificationId: string, version: number) {
    return request<SpecificationRevisionResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/revisions/${version}`, {}, workspaceId);
  },
  restoreSpecificationRevision(workspaceId: string, specificationId: string, version: number, etag: string) {
    return request<SpecificationResponse>(`/api/specx/specifications/${encodeURIComponent(specificationId)}/revisions/${version}/restore`, { method: "POST", headers: { "if-match": etag } }, workspaceId);
  },
};
