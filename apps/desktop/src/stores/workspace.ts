import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { loadPanelSession } from "../lib/core";
import { specxApi } from "../lib/backend/client";
import type { WorkspaceResponse } from "../types/api";
import type { Catalog, PanelTab, Specification, SpecificationRevision } from "../types/domain";
import { translate } from "@/i18n";

export const useWorkspaceStore = defineStore("workspace", () => {
  const authenticated = ref<boolean | null>(null);
  const migrationReady = ref(true);
  const loading = ref(true);
  const error = ref("");
  const workspaces = ref<WorkspaceResponse[]>([]);
  const catalog = ref<Catalog | null>(null);
  const tabs = ref<PanelTab[]>([]);
  const activeTabId = ref("");
  const activeServiceId = ref<string | null>(null);
  const pendingCloseTabId = ref<string | null>(null);
  const draftRevision = ref(0);
  type DraftStatus = "dirty" | "saving" | "saved" | "error" | "conflict";
  type SpecificationDraftState = {
    specificationId: string;
    title: string;
    content: string;
    baseEtag: string;
    baseVersion: number;
    status: DraftStatus;
    error: string;
    remote?: Pick<Specification, "title" | "content" | "version" | "etag" | "updatedAt">;
  };
  const drafts = ref<Record<string, SpecificationDraftState>>({});
  const revisions = ref<Record<string, SpecificationRevision[]>>({});
  const revisionsLoading = ref<Record<string, boolean>>({});
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

  function draftStorageKey(workspaceId: string) {
    return `specx-specification-drafts-v1:${workspaceId}`;
  }

  function persistDrafts() {
    const workspaceId = catalog.value?.workspace.id;
    if (!workspaceId) return;
    const serializable = Object.fromEntries(Object.entries(drafts.value).map(([id, draft]) => [id, { ...draft, status: draft.status === "saving" ? "dirty" : draft.status }]));
    try {
      localStorage.setItem(draftStorageKey(workspaceId), JSON.stringify(serializable));
    } catch {
      /* Optional browser storage. */
    }
  }

  function restoreDrafts(next: Catalog) {
    let saved: Record<string, Partial<SpecificationDraftState>> = {};
    try {
      const value = JSON.parse(localStorage.getItem(draftStorageKey(next.workspace.id)) ?? "{}");
      if (value && typeof value === "object") saved = value;
    } catch {
      saved = {};
    }
    const nextDrafts: Record<string, SpecificationDraftState> = {};
    for (const specification of next.specifications) {
      const savedDraft = saved[specification.id];
      if (!savedDraft || typeof savedDraft.title !== "string" || typeof savedDraft.content !== "string") continue;
      if (savedDraft.title === specification.title && savedDraft.content === specification.content) continue;
      nextDrafts[specification.id] = {
        specificationId: specification.id,
        title: savedDraft.title,
        content: savedDraft.content,
        baseEtag: typeof savedDraft.baseEtag === "string" ? savedDraft.baseEtag : specification.etag,
        baseVersion: typeof savedDraft.baseVersion === "number" ? savedDraft.baseVersion : specification.version,
        status: "dirty",
        error: "",
      };
    }
    drafts.value = nextDrafts;
    draftRevision.value++;
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
      restoreDrafts(next);
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
    migrationReady.value = true;
    try {
      const migration = await specxApi.migrationRetry();
      migrationReady.value = migration.ready;
      if (!migration.ready) {
        error.value = translate("app.storageNotReady");
        return;
      }
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

  function specificationDraft(specificationId: string) {
    return drafts.value[specificationId];
  }

  function beginSpecificationEdit(specificationId: string) {
    const specification = catalog.value?.specifications.find((item) => item.id === specificationId);
    if (!specification) return undefined;
    const existing = drafts.value[specificationId];
    if (existing) return existing;
    const draft: SpecificationDraftState = {
      specificationId,
      title: specification.title,
      content: specification.content,
      baseEtag: specification.etag,
      baseVersion: specification.version,
      status: "saved",
      error: "",
    };
    drafts.value = { ...drafts.value, [specificationId]: draft };
    draftRevision.value++;
    persistDrafts();
    return draft;
  }

  function updateSpecificationDraft(specificationId: string, values: { title: string; content: string }) {
    const draft = drafts.value[specificationId] ?? beginSpecificationEdit(specificationId);
    if (!draft) return;
    const specification = catalog.value?.specifications.find((item) => item.id === specificationId);
    const dirty = Boolean(specification && (values.title !== specification.title || values.content !== specification.content));
    drafts.value = {
      ...drafts.value,
      [specificationId]: { ...draft, ...values, status: dirty ? "dirty" : "saved", error: "", remote: undefined },
    };
    draftRevision.value++;
    persistDrafts();
  }

  function hasUnsavedSpecification(specificationId: string) {
    const draft = drafts.value[specificationId];
    return Boolean(draft && (draft.status === "dirty" || draft.status === "error" || draft.status === "conflict"));
  }

  async function saveSpecification(specificationId: string) {
    const catalogValue = catalog.value;
    const draft = drafts.value[specificationId];
    const specification = catalogValue?.specifications.find((item) => item.id === specificationId);
    if (!catalogValue || !draft || !specification || !hasUnsavedSpecification(specificationId)) return true;
    drafts.value = { ...drafts.value, [specificationId]: { ...draft, status: "saving", error: "" } };
    draftRevision.value++;
    persistDrafts();
    try {
      const saved = await specxApi.updateSpecification(catalogValue.workspace.id, specificationId, { title: draft.title.trim(), content: draft.content }, draft.baseEtag);
      catalogValue.specifications = catalogValue.specifications.map((item) => (item.id === specificationId ? { ...item, ...saved, kind: "markdown" } : item));
      tabs.value = tabs.value.map((tab) => (tab.specificationId === specificationId ? { ...tab, title: saved.title } : tab));
      delete drafts.value[specificationId];
      drafts.value = { ...drafts.value };
      draftRevision.value++;
      persistDrafts();
      return true;
    } catch (cause) {
      const status = cause && typeof cause === "object" && "status" in cause ? Number(cause.status) : 0;
      if (status === 412) {
        try {
          await refresh();
        } catch {
          /* Preserve the local draft even when the conflict refresh fails. */
        }
        const remote = catalog.value?.specifications.find((item) => item.id === specificationId);
        drafts.value = {
          ...drafts.value,
          [specificationId]: { ...draft, status: "conflict", error: "", remote: remote ? { title: remote.title, content: remote.content, version: remote.version, etag: remote.etag, updatedAt: remote.updatedAt } : undefined },
        };
      } else {
        drafts.value = { ...drafts.value, [specificationId]: { ...draft, status: "error", error: cause instanceof Error ? cause.message : translate("specification.saveFailed") } };
      }
      draftRevision.value++;
      persistDrafts();
      return false;
    }
  }

  function discardSpecificationDraft(specificationId: string) {
    delete drafts.value[specificationId];
    drafts.value = { ...drafts.value };
    draftRevision.value++;
    persistDrafts();
  }

  async function loadSpecificationRevisions(specificationId: string) {
    const workspaceId = catalog.value?.workspace.id;
    if (!workspaceId) return [];
    revisionsLoading.value = { ...revisionsLoading.value, [specificationId]: true };
    try {
      const page = await specxApi.listSpecificationRevisions(workspaceId, specificationId);
      revisions.value = { ...revisions.value, [specificationId]: page.items };
      return page.items;
    } finally {
      revisionsLoading.value = { ...revisionsLoading.value, [specificationId]: false };
    }
  }

  function specificationRevisions(specificationId: string) {
    return revisions.value[specificationId] ?? [];
  }

  function specificationRevisionsLoading(specificationId: string) {
    return revisionsLoading.value[specificationId] ?? false;
  }

  async function restoreSpecificationRevision(specificationId: string, version: number) {
    const catalogValue = catalog.value;
    const specification = catalogValue?.specifications.find((item) => item.id === specificationId);
    if (!catalogValue || !specification) return false;
    const restored = await specxApi.restoreSpecificationRevision(catalogValue.workspace.id, specificationId, version, specification.etag);
    catalogValue.specifications = catalogValue.specifications.map((item) => (item.id === specificationId ? { ...item, ...restored, kind: "markdown" } : item));
    delete drafts.value[specificationId];
    drafts.value = { ...drafts.value };
    draftRevision.value++;
    persistDrafts();
    await loadSpecificationRevisions(specificationId);
    return true;
  }

  async function deleteSpecification(specificationId: string) {
    const catalogValue = catalog.value;
    const specification = catalogValue?.specifications.find((item) => item.id === specificationId);
    if (!catalogValue || !specification) return false;
    await specxApi.deleteSpecification(catalogValue.workspace.id, specificationId, specification.etag);
    catalogValue.specifications = catalogValue.specifications.filter((item) => item.id !== specificationId);
    delete drafts.value[specificationId];
    delete revisions.value[specificationId];
    drafts.value = { ...drafts.value };
    revisions.value = { ...revisions.value };
    draftRevision.value++;
    persistDrafts();
    for (const tab of tabs.value.filter((item) => item.specificationId === specificationId)) closeTab(tab.id, true);
    return true;
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

  function closeTab(id: string, force = false) {
    const tab = tabs.value.find((item) => item.id === id);
    if (!force && tab?.specificationId && hasUnsavedSpecification(tab.specificationId)) {
      pendingCloseTabId.value = id;
      return false;
    }
    const index = tabs.value.findIndex((tab) => tab.id === id);
    tabs.value = tabs.value.filter((tab) => tab.id !== id);
    if (activeTabId.value === id) activeTabId.value = tabs.value[Math.max(0, index - 1)]?.id ?? "";
    if (pendingCloseTabId.value === id) pendingCloseTabId.value = null;
    return true;
  }

  async function resolvePendingClose(action: "save" | "discard" | "cancel") {
    const id = pendingCloseTabId.value;
    if (!id) return false;
    if (action === "cancel") {
      pendingCloseTabId.value = null;
      return false;
    }
    const tab = tabs.value.find((item) => item.id === id);
    if (action === "save" && tab?.specificationId && !(await saveSpecification(tab.specificationId))) return false;
    if (action === "discard" && tab?.specificationId) discardSpecificationDraft(tab.specificationId);
    return closeTab(id, true);
  }

  return {
    authenticated,
    migrationReady,
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
    pendingCloseTabId,
    resolvePendingClose,
    specificationDraft,
    beginSpecificationEdit,
    updateSpecificationDraft,
    hasUnsavedSpecification,
    saveSpecification,
    discardSpecificationDraft,
    specificationRevisions,
    specificationRevisionsLoading,
    loadSpecificationRevisions,
    restoreSpecificationRevision,
    deleteSpecification,
  };
});
