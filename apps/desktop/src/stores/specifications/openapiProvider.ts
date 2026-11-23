import { ref } from "vue";
import { translate } from "@/i18n";
import { specxApi } from "@/lib/backend/client";
import type { OpenApiEnvironmentInput, OpenApiProviderResponse, OpenApiSettings } from "@/types/api";

export function createOpenApiProviderState(workspaceId: () => string | undefined) {
  const openapiProviders = ref<Record<string, OpenApiProviderResponse>>({});

  function reset() {
    openapiProviders.value = {};
  }

  function openApiProviderFor(serviceId: string) {
    return openapiProviders.value[serviceId] ?? null;
  }

  function setProvider(serviceId: string, provider: OpenApiProviderResponse) {
    openapiProviders.value = { ...openapiProviders.value, [serviceId]: provider };
  }

  function patchProvider(id: string, serviceId: string, patch: (current: OpenApiProviderResponse) => Partial<OpenApiProviderResponse>) {
    if (workspaceId() !== id) return;
    const current = openApiProviderFor(serviceId);
    if (current) setProvider(serviceId, { ...current, ...patch(current) });
  }

  function requireWorkspaceId() {
    const id = workspaceId();
    if (!id) throw new Error(translate("apiError.forbidden"));
    return id;
  }

  async function loadOpenApiProvider(serviceId: string) {
    const id = workspaceId();
    if (!id) return null;
    const provider = await specxApi.getOpenApiProvider(id, serviceId);
    if (workspaceId() === id) setProvider(serviceId, provider);
    return provider;
  }

  async function updateOpenApiSettings(serviceId: string, settings: Omit<OpenApiSettings, "updatedAt" | "mockEnabled">) {
    const id = requireWorkspaceId();
    const saved = await specxApi.updateOpenApiSettings(id, serviceId, settings);
    patchProvider(id, serviceId, () => ({ settings: saved }));
    return saved;
  }

  async function enableOpenApiMock(serviceId: string) {
    const id = requireWorkspaceId();
    const result = await specxApi.enableOpenApiMock(id, serviceId);
    patchProvider(id, serviceId, (current) => ({ settings: { ...current.settings, mockEnabled: result.enabled } }));
    return result;
  }

  async function disableOpenApiMock(serviceId: string) {
    const id = workspaceId();
    if (!id) return;
    await specxApi.disableOpenApiMock(id, serviceId);
    patchProvider(id, serviceId, (current) => ({ settings: { ...current.settings, mockEnabled: false } }));
  }

  async function createOpenApiEnvironment(serviceId: string, input: OpenApiEnvironmentInput & { id: string; slug?: string }) {
    const id = requireWorkspaceId();
    const created = await specxApi.createOpenApiEnvironment(id, serviceId, input);
    patchProvider(id, serviceId, (current) => ({ environments: [...current.environments.filter((item) => item.id !== created.id), created] }));
    return created;
  }

  async function updateOpenApiEnvironment(serviceId: string, environmentId: string, input: OpenApiEnvironmentInput & { slug: string }) {
    const id = requireWorkspaceId();
    const updated = await specxApi.updateOpenApiEnvironment(id, serviceId, environmentId, input);
    patchProvider(id, serviceId, (current) => ({ environments: current.environments.map((item) => (item.id === environmentId ? updated : item)) }));
    return updated;
  }

  async function deleteOpenApiEnvironment(serviceId: string, environmentId: string) {
    const id = workspaceId();
    if (!id) return;
    await specxApi.deleteOpenApiEnvironment(id, serviceId, environmentId);
    patchProvider(id, serviceId, (current) => ({
      environments: current.environments.filter((item) => item.id !== environmentId),
      settings: current.settings.defaultEnvironmentId === environmentId ? { ...current.settings, defaultEnvironmentId: null } : current.settings,
    }));
  }

  async function resolveOpenApiEnvironment(serviceId: string, environmentId: string) {
    return specxApi.resolveOpenApiEnvironment(requireWorkspaceId(), serviceId, environmentId);
  }

  return {
    reset,
    openapiProviders,
    loadOpenApiProvider,
    openApiProviderFor,
    updateOpenApiSettings,
    enableOpenApiMock,
    disableOpenApiMock,
    createOpenApiEnvironment,
    updateOpenApiEnvironment,
    deleteOpenApiEnvironment,
    resolveOpenApiEnvironment,
  };
}
