import { uuid } from "../common/utils";
import type { Catalog, Group, Service, Specification } from "../../types/domain";
import type { AuthResponse, CatalogResponse, WorkspaceListResponse, WorkspaceResponse } from "../../types/api";
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
    },
  ) {
    return request<unknown>(
      "/api/specx/specifications",
      {
        method: "POST",
        headers: { "idempotency-key": idempotencyKey("specification") },
        body: input,
      },
      workspaceId,
    );
  },
};
