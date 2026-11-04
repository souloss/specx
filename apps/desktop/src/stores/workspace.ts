import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { loadPanelSession } from "../lib/core";
import { specxApi } from "../lib/backend/client";
import type { WorkspaceResponse } from "../types/api";
import type { Catalog, PanelTab } from "../types/domain";
import { translate } from "@/i18n";

export const useWorkspaceStore = defineStore("workspace", () => {
  const authenticated = ref<boolean | null>(null);
  const loading = ref(true);
  const error = ref("");
  const workspaces = ref<WorkspaceResponse[]>([]);
  const catalog = ref<Catalog | null>(null);
  const tabs = ref<PanelTab[]>([]);
  const activeTabId = ref("");
  const activeServiceId = ref<string | null>(null);
  let loadRevision = 0;
  watch(
    authenticated,
    (value) => {
      if (value === false) loadRevision++;
    },
    { flush: "sync" },
  );

  const activeTab = computed(() => tabs.value.find((tab) => tab.id === activeTabId.value) ?? tabs.value[0]);
  const activeService = computed(() => catalog.value?.services.find((service) => service.id === activeServiceId.value) ?? null);
  watch(activeTabId, (id) => {
    activeServiceId.value = tabs.value.find((tab) => tab.id === id)?.serviceId ?? null;
  });
  watch(
    [tabs, activeTabId],
    () => {
      if (!catalog.value) return;
      try {
        localStorage.setItem(`specx-panel-session-v1:${catalog.value.workspace.id}`, JSON.stringify({ tabs: tabs.value, activeTabId: activeTabId.value }));
      } catch {
        /* Optional browser storage. */
      }
    },
    { deep: true },
  );

  function restorePanelState(next: Catalog) {
    const panel = loadPanelSession(next.workspace.id, next.services, next.specifications);
    tabs.value = panel.tabs;
    activeTabId.value = panel.activeTabId;
    activeServiceId.value = panel.tabs.find((tab) => tab.id === panel.activeTabId)?.serviceId ?? next.services[0]?.id ?? null;
  }

  async function load(workspaceId?: string) {
    const revision = ++loadRevision;
    const before = catalog.value;
    loading.value = !catalog.value;
    error.value = "";
    try {
      const availableWorkspaces = (await specxApi.listWorkspaces()).items;
      const next = await specxApi.loadCatalog(workspaceId, availableWorkspaces);
      if (revision !== loadRevision) return;
      // A refresh begun before a create response must not erase that new item.
      if (before?.workspace.id === next.workspace.id && catalog.value?.workspace.id === next.workspace.id) {
        const knownGroups = new Set([...before.groups, ...next.groups].map((item) => item.id));
        const knownServices = new Set([...before.services, ...next.services].map((item) => item.id));
        next.groups.push(...catalog.value.groups.filter((item) => !knownGroups.has(item.id)));
        next.services.push(...catalog.value.services.filter((item) => !knownServices.has(item.id)));
      }
      const sameWorkspace = catalog.value?.workspace.id === next.workspace.id;
      const previousTabs = tabs.value.filter((tab) => next.services.some((service) => service.id === tab.serviceId) && (tab.kind !== "specification" || next.specifications.some((specification) => specification.id === tab.specificationId)));
      catalog.value = next;
      workspaces.value = availableWorkspaces;
      if (sameWorkspace) {
        tabs.value = previousTabs.map((tab) => ({ ...tab, title: tab.kind === "specification" ? next.specifications.find((item) => item.id === tab.specificationId)!.title : next.services.find((item) => item.id === tab.serviceId)!.name }));
        if (!tabs.value.some((tab) => tab.id === activeTabId.value)) activeTabId.value = tabs.value[0]?.id ?? "";
        activeServiceId.value = tabs.value.find((tab) => tab.id === activeTabId.value)?.serviceId ?? null;
      } else restorePanelState(next);
    } catch (cause) {
      if (revision !== loadRevision) return;
      error.value = cause instanceof Error ? cause.message : translate("app.loadFailed");
      throw cause;
    } finally {
      if (revision === loadRevision) loading.value = false;
    }
  }

  async function initialize() {
    loading.value = true;
    try {
      const status = await specxApi.authCheck();
      authenticated.value = status.authenticated;
      if (status.authenticated) await load();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : translate("app.connectionFailed");
    } finally {
      loading.value = false;
    }
  }

  async function login(password: string) {
    await specxApi.login(password);
    authenticated.value = true;
    await load();
  }

  async function refresh(preferredServiceId?: string) {
    const workspaceId = catalog.value?.workspace.id;
    await load(workspaceId);
    if (preferredServiceId && catalog.value?.services.some((service) => service.id === preferredServiceId)) {
      activeServiceId.value = preferredServiceId;
      openOverview(preferredServiceId);
    }
  }

  function selectService(serviceId: string) {
    activeServiceId.value = serviceId;
    openOverview(serviceId);
  }

  function openOverview(serviceId: string) {
    const id = `overview-${serviceId}`;
    if (!tabs.value.some((tab) => tab.id === id)) {
      const service = catalog.value?.services.find((item) => item.id === serviceId);
      tabs.value.push({ id, kind: "overview", serviceId, title: service?.name ?? translate("service.overview") });
    }
    activeTabId.value = id;
  }

  function openSpecification(specificationId: string) {
    const specification = catalog.value?.specifications.find((item) => item.id === specificationId);
    if (!specification) return;
    const id = `specification-${specification.id}`;
    if (!tabs.value.some((tab) => tab.id === id)) {
      tabs.value.push({ id, kind: "specification", serviceId: specification.serviceId, specificationId, title: specification.title });
    }
    activeServiceId.value = specification.serviceId;
    activeTabId.value = id;
  }

  function closeTab(id: string) {
    const index = tabs.value.findIndex((tab) => tab.id === id);
    tabs.value = tabs.value.filter((tab) => tab.id !== id);
    if (activeTabId.value === id) activeTabId.value = tabs.value[Math.max(0, index - 1)]?.id ?? "";
  }

  return {
    authenticated,
    loading,
    error,
    workspaces,
    catalog,
    tabs,
    activeTabId,
    activeServiceId,
    activeTab,
    activeService,
    initialize,
    login,
    load,
    refresh,
    selectService,
    openOverview,
    openSpecification,
    closeTab,
  };
});
