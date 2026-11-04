import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { useWorkspaceStore } from "./workspace";
import { specxApi, toService } from "@/lib/backend/client";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { UNGROUPED_GROUP_ID } from "@/lib/core";
import { translate } from "@/i18n";

export type CreateDraft = { id: string; workspaceId: string; kind: "group" | "resource"; name: string; parentId: string | null; status: "saving" | "failed"; error: string };
const STORAGE_KEY = "specx-create-drafts-v1";

/** Pending creations are an overlay, never a rollback snapshot of shared data.
 * Retries keep both the resource ID and idempotency key, including after reload. */
export const useCreateQueueStore = defineStore("createQueue", () => {
  const workspace = useWorkspaceStore();
  const drafts = ref<CreateDraft[]>([]);
  try {
    const saved: unknown = JSON.parse(safeLocalStorageGet(STORAGE_KEY) ?? "[]");
    if (Array.isArray(saved))
      drafts.value = saved
        .filter((item): item is CreateDraft => item && typeof item.id === "string" && typeof item.workspaceId === "string" && typeof item.name === "string" && (item.parentId === null || typeof item.parentId === "string") && ["group", "resource"].includes(item.kind))
        .map((item) => ({ ...item, status: "failed", error: "" }));
  } catch {
    /* Optional storage may contain stale data. */
  }
  const current = computed(() => drafts.value.filter((item) => item.workspaceId === workspace.catalog?.workspace.id));
  const running = new Map<string, Promise<void>>();
  let sessionRevision = 0;
  watch(
    () => workspace.authenticated,
    (authenticated) => {
      if (authenticated === false) sessionRevision++;
    },
    { flush: "sync" },
  );
  watch(drafts, (value) => safeLocalStorageSet(STORAGE_KEY, JSON.stringify(value)), { deep: true, flush: "sync" });
  watch(
    () => workspace.catalog,
    (catalog) => {
      if (!catalog || !drafts.value.length) return;
      // A response can be lost after the server commits. A later catalog load
      // confirms that resource without issuing another write.
      const confirmedGroups = new Set(catalog.groups.map((item) => item.id));
      const confirmedServices = new Set(catalog.services.map((item) => item.id));
      const remaining = drafts.value.filter((item) => item.workspaceId !== catalog.workspace.id || running.has(item.id) || !(item.kind === "group" ? confirmedGroups : confirmedServices).has(item.id));
      if (remaining.length !== drafts.value.length) drafts.value = remaining;
    },
    { immediate: true },
  );

  function enqueue(input: Pick<CreateDraft, "id" | "workspaceId" | "kind" | "name" | "parentId">) {
    if (drafts.value.some((item) => item.id === input.id)) return;
    drafts.value.push({ ...input, status: "saving", error: "" });
    void retry(input.id);
  }

  function retry(id: string): Promise<void> {
    if (running.has(id)) return running.get(id)!;
    const draft = drafts.value.find((item) => item.id === id);
    if (!draft || workspace.authenticated === false || draft.workspaceId !== workspace.catalog?.workspace.id) return Promise.resolve();
    draft.status = "saving";
    draft.error = "";
    const revision = sessionRevision;
    const operation = (async () => {
      try {
        const key = `create-${draft.kind}-${draft.id}`;
        const result =
          draft.kind === "group"
            ? await specxApi.createGroup(draft.workspaceId, { id: draft.id, name: draft.name, parentId: draft.parentId }, key)
            : await specxApi.createService(draft.workspaceId, { id: draft.id, name: draft.name, slug: draft.id, description: "", groupId: draft.parentId, tagIds: [] }, key);
        // A sign-out invalidates this UI session even if a request finishes later.
        if (revision !== sessionRevision) throw new Error(translate("apiError.unauthorized"));
        const catalog = workspace.catalog;
        if (catalog?.workspace.id === draft.workspaceId) {
          if (draft.kind === "group") {
            const group = result as Awaited<ReturnType<typeof specxApi.createGroup>>;
            workspace.catalog = { ...catalog, groups: [...catalog.groups.filter((item) => item.id !== group.id), group] };
          } else {
            const service = toService(result as Awaited<ReturnType<typeof specxApi.createService>>);
            const groups = service.groupId === UNGROUPED_GROUP_ID && !catalog.groups.some((item) => item.id === UNGROUPED_GROUP_ID) ? [...catalog.groups, { id: UNGROUPED_GROUP_ID, name: translate("common.ungrouped"), parentId: null }] : catalog.groups;
            workspace.catalog = { ...catalog, groups, services: [...catalog.services.filter((item) => item.id !== service.id), service] };
          }
        }
        drafts.value = drafts.value.filter((item) => item.id !== id);
      } catch (error) {
        draft.status = "failed";
        draft.error = error instanceof Error ? error.message : translate("tree.operationFailed");
      } finally {
        running.delete(id);
      }
    })();
    running.set(id, operation);
    return operation;
  }

  function discard(id: string) {
    if (running.has(id)) return;
    drafts.value = drafts.value.filter((item) => item.id !== id);
  }
  return { current, enqueue, retry, discard };
});
