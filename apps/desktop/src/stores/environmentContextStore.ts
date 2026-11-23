import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";

export type EnvironmentContextOption = { id: string; name: string; scope: "global" | "openapi" };
export type GlobalEnvironmentVariable = { key: string; value: string; sensitive: boolean; enabled: boolean };
export type GlobalEnvironment = { requestMode?: "inherit" | "browser" | "agent"; id: string; name: string; baseUrl: string; variables: GlobalEnvironmentVariable[] };

const activeStorageKey = "specx-active-environment";
const globalActiveStorageKey = "specx-active-global-environment";
const globalEnvironmentsStorageKey = "specx-global-environments";
const scopedSelectionStorageKey = "specx-scoped-environment-selections";

function readGlobalEnvironments(): GlobalEnvironment[] {
  const raw = safeLocalStorageGet(globalEnvironmentsStorageKey);
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((item): GlobalEnvironment[] => {
      if (!item || typeof item !== "object") return [];
      const candidate = item as Partial<GlobalEnvironment>;
      if (typeof candidate.id !== "string" || typeof candidate.name !== "string" || !Array.isArray(candidate.variables)) return [];
      const variables = candidate.variables.flatMap((variable): GlobalEnvironmentVariable[] => {
        if (!variable || typeof variable !== "object") return [];
        const value = variable as Partial<GlobalEnvironmentVariable>;
        return typeof value.key === "string" ? [{ key: value.key, value: typeof value.value === "string" ? value.value : "", sensitive: Boolean(value.sensitive), enabled: value.enabled !== false }] : [];
      });
      return [{ requestMode: candidate.requestMode === "browser" || candidate.requestMode === "agent" ? candidate.requestMode : "inherit", id: candidate.id, name: candidate.name, baseUrl: typeof candidate.baseUrl === "string" ? candidate.baseUrl : "", variables }];
    });
  } catch {
    return [];
  }
}

function readScopedSelections(): Record<string, string | null> {
  const raw = safeLocalStorageGet(scopedSelectionStorageKey);
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Record<string, string | null>) : {};
  } catch {
    return {};
  }
}

/** The selected environment is a platform context. Providers register their
 * available options; request editors only consume the selected id. */
export const useEnvironmentContextStore = defineStore("environmentContext", () => {
  const sourceOptions = ref<Record<string, EnvironmentContextOption[]>>({});
  const activeSourceId = ref<string | null>(null);
  const scopedSelections = ref<Record<string, string | null>>(readScopedSelections());
  const options = computed(() => {
    const unique = new Map<string, EnvironmentContextOption>();
    for (const source of Object.values(sourceOptions.value)) for (const item of source) if (item.id && !unique.has(item.id)) unique.set(item.id, item);
    return [...unique.values()];
  });
  const activeId = computed(() => (activeSourceId.value ? (scopedSelections.value[activeSourceId.value] ?? null) : null));
  const globalEnvironments = ref<GlobalEnvironment[]>(readGlobalEnvironments());
  const activeGlobalId = ref<string | null>(safeLocalStorageGet(globalActiveStorageKey));
  const settingsOpen = ref(false);
  const activeOptions = computed(() => (activeSourceId.value ? (sourceOptions.value[activeSourceId.value] ?? []) : []));
  const active = computed(() => activeOptions.value.find((item) => item.id === activeId.value) ?? null);
  const activeGlobal = computed(() => globalEnvironments.value.find((item) => item.id === activeGlobalId.value) ?? null);
  const activeSelectionLabel = computed(() => [activeGlobal.value?.name, active.value?.name].filter(Boolean).join(" · "));

  function persistGlobals() {
    const persisted = globalEnvironments.value.map((environment) => ({
      ...environment,
      variables: environment.variables.map((variable) => ({ ...variable, value: variable.sensitive ? "" : variable.value })),
    }));
    safeLocalStorageSet(globalEnvironmentsStorageKey, JSON.stringify(persisted));
  }

  function persistSelections() {
    safeLocalStorageSet(scopedSelectionStorageKey, JSON.stringify(scopedSelections.value));
  }

  function registerOptions(source: string, next: EnvironmentContextOption[], defaultId: string | null = null) {
    if (!source) return;
    sourceOptions.value = { ...sourceOptions.value, [source]: next };
    if (!activeSourceId.value) activeSourceId.value = source;
    const selected = scopedSelections.value[source];
    const fallback = next.find((item) => item.id === defaultId)?.id ?? (source.startsWith("openapi:") ? next[0]?.id : null) ?? null;
    if (!next.some((item) => item.id === selected) && selected !== fallback) {
      scopedSelections.value = { ...scopedSelections.value, [source]: fallback };
    }
    persistSelections();
  }

  function activateSource(source: string | null) {
    activeSourceId.value = source;
  }

  function unregisterOptions(source: string) {
    if (!Object.hasOwn(sourceOptions.value, source)) return;
    const next = { ...sourceOptions.value };
    delete next[source];
    sourceOptions.value = next;
    if (activeSourceId.value === source) activeSourceId.value = null;
  }

  /** Compatibility for callers that own the complete option set. */
  function setOptions(next: EnvironmentContextOption[]) {
    registerOptions("legacy", next);
  }

  function select(id: string | null) {
    if (!activeSourceId.value) return;
    selectFor(activeSourceId.value, id);
    safeLocalStorageSet(activeStorageKey, activeId.value ?? "");
  }

  function selectFor(source: string, id: string | null) {
    const available = sourceOptions.value[source] ?? [];
    if (source.startsWith("openapi:") && !available.some((item) => item.id === id)) return;
    scopedSelections.value = { ...scopedSelections.value, [source]: id && available.some((item) => item.id === id) ? id : null };
    persistSelections();
  }

  function selectionFor(source: string): string | null {
    return scopedSelections.value[source] ?? null;
  }

  function selectChoice(choice: string) {
    if (choice.startsWith("global:")) {
      const id = choice.slice("global:".length);
      selectGlobal(id === "none" ? null : id);
      return;
    }
    if (choice.startsWith("openapi:")) {
      const id = choice.slice("openapi:".length);
      select(id === "none" ? null : id);
    }
  }

  function selectGlobal(id: string | null) {
    activeGlobalId.value = id && globalEnvironments.value.some((item) => item.id === id) ? id : null;
    safeLocalStorageSet(globalActiveStorageKey, activeGlobalId.value ?? "");
  }

  function addGlobal(input: { requestMode?: "inherit" | "browser" | "agent"; name?: string; baseUrl?: string; variables?: GlobalEnvironmentVariable[]; activate?: boolean }): GlobalEnvironment {
    const environment: GlobalEnvironment = {
      id: `global-${globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`}`,
      name: input.name?.trim() || `Environment ${globalEnvironments.value.length + 1}`,
      baseUrl: input.baseUrl ?? "",
      requestMode: input.requestMode ?? "inherit",
      variables: input.variables ? input.variables.map((variable) => ({ ...variable })) : [],
    };
    globalEnvironments.value = [...globalEnvironments.value, environment];
    persistGlobals();
    if (input.activate !== false) selectGlobal(environment.id);
    return environment;
  }

  function updateGlobal(id: string, patch: Partial<Pick<GlobalEnvironment, "name" | "baseUrl" | "variables" | "requestMode">>) {
    globalEnvironments.value = globalEnvironments.value.map((item) => (item.id === id ? { ...item, ...patch, name: patch.name?.trim() || item.name, baseUrl: patch.baseUrl ?? item.baseUrl, variables: patch.variables?.map((variable) => ({ ...variable })) ?? item.variables } : item));
    persistGlobals();
  }

  function deleteGlobal(id: string) {
    globalEnvironments.value = globalEnvironments.value.filter((item) => item.id !== id);
    persistGlobals();
    if (activeGlobalId.value === id) selectGlobal(globalEnvironments.value[0]?.id ?? null);
  }

  function openSettings() {
    settingsOpen.value = true;
  }

  return {
    options,
    activeOptions,
    activateSource,
    activeSourceId,
    activeId,
    active,
    activeSelectionLabel,
    globalEnvironments,
    activeGlobalId,
    activeGlobal,
    settingsOpen,
    setOptions,
    registerOptions,
    unregisterOptions,
    select,
    selectFor,
    selectionFor,
    selectChoice,
    selectGlobal,
    addGlobal,
    updateGlobal,
    deleteGlobal,
    openSettings,
  };
});
