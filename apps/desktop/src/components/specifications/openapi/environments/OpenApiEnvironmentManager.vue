<script setup lang="ts">
import { computed, watch, ref } from "vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { translate as t } from "@/i18n";
import { createId } from "@/lib/core";
import { downloadText } from "@/lib/transfer/downloadText";
import { environmentDraft, environmentExport, environmentInput, type EnvironmentDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";
import { useEnvironmentContextStore } from "@/stores/environmentContextStore";
import { useEnvironmentLocalValues } from "@/stores/environmentLocalValues";
import { useWorkspaceStore } from "@/stores/workspace";
import EnvironmentEditor from "./EnvironmentEditor.vue";
import EnvironmentManager from "../../../environments/EnvironmentManager.vue";

const props = defineProps<{ serviceId: string }>();
const context = useEnvironmentContextStore();
const workspace = useWorkspaceStore();
const localValues = useEnvironmentLocalValues();
const selectedId = ref<string | null>(null);
const drafts = ref<Record<string, EnvironmentDraft>>({});
const busy = ref(false);
const saved = ref(false);
const error = ref("");
const deleteId = ref<string | null>(null);
const open = computed({
  get: () => context.settingsOpen,
  set: (value) => {
    if (!busy.value) context.settingsOpen = value;
  },
});
const environments = computed(() => workspace.openApiProviderFor(props.serviceId)?.environments ?? []);
const items = computed(() => [...environments.value.map((item) => drafts.value[item.id] ?? item), ...Object.values(drafts.value).filter((item) => !environments.value.some((environment) => environment.id === item.id))]);
const deleteItem = computed(() => items.value.find((item) => item.id === deleteId.value));
const draft = computed({
  get: () => (selectedId.value ? drafts.value[selectedId.value] : undefined),
  set: (value) => {
    if (value) drafts.value[value.id] = value;
  },
});
const localScope = (id: string) => `${workspace.catalog?.workspace.id}:${props.serviceId}:${id}`;

function getDraft(id: string) {
  if (Object.hasOwn(drafts.value, id)) return drafts.value[id];
  const item = environments.value.find((value) => value.id === id);
  return item ? environmentDraft(item, localValues.values(localScope(id))) : undefined;
}

function select(id: string | null) {
  const value = id ? getDraft(id) : undefined;
  selectedId.value = value?.id ?? null;
  if (value) drafts.value[value.id] = value;
  saved.value = false;
  error.value = "";
}

watch(
  open,
  (value) => {
    if (value) select(context.selectionFor(`openapi:${props.serviceId}`) ?? selectedId.value ?? items.value[0]?.id ?? null);
    else deleteId.value = null;
  },
  { immediate: true },
);
watch(environments, () => {
  if (open.value && !selectedId.value && items.value[0]) select(items.value[0].id);
});

function add() {
  const id = createId("environment");
  drafts.value[id] = { id, name: t("environmentContext.newEnvironment"), slug: "", baseUrl: "", requestMode: "inherit", servers: [], variables: [] };
  select(id);
}

function duplicate(id: string) {
  const source = getDraft(id);
  if (!source) return;
  const copyId = createId("environment");
  const copyName = t("environmentContext.copyName", { name: source.name });
  drafts.value[copyId] = {
    ...source,
    id: copyId,
    name: copyName,
    slug: "",
    servers: source.servers?.map((row) => ({ ...row })),
    variables: source.variables.map(({ key, value, localValue, sensitive, enabled }) => ({ key, sensitive, enabled, localValue: sensitive ? undefined : (localValue ?? value ?? undefined) })),
  };
  select(copyId);
}

function exportOne(id: string) {
  const value = getDraft(id);
  if (!value) return;
  const data = { format: "specx-openapi-environments", version: 1, environments: [environmentExport(value)] };
  downloadText(JSON.stringify(data, null, 2), `${value.slug || value.id}-environment.json`, "application/json");
}

async function save(close: boolean) {
  const value = draft.value;
  if (!value || busy.value) return;
  busy.value = true;
  error.value = "";
  saved.value = false;
  try {
    const input = environmentInput(value);
    const result = value.slug ? await workspace.updateOpenApiEnvironment(props.serviceId, value.id, { ...input, slug: value.slug }) : await workspace.createOpenApiEnvironment(props.serviceId, { ...input, id: value.id, slug: value.id });
    localValues.replace(
      localScope(result.id),
      value.variables.map((row) => ({ ...row, key: row.key.trim() })),
    );
    drafts.value[result.id] = environmentDraft(result, localValues.values(localScope(result.id)));
    saved.value = true;
    if (close) context.settingsOpen = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.saveFailed");
  } finally {
    busy.value = false;
  }
}

async function remove() {
  const id = deleteId.value;
  if (!id || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    if (environments.value.some((item) => item.id === id)) await workspace.deleteOpenApiEnvironment(props.serviceId, id);
    localValues.replace(localScope(id), []);
    localValues.replaceScriptValues(localScope(id), {});
    delete drafts.value[id];
    deleteId.value = null;
    if (selectedId.value === id) select(items.value[0]?.id ?? null);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.saveFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <EnvironmentManager
    v-model:open="open"
    :items="items"
    :selected-id="selectedId"
    :busy="busy"
    content-class="openapi-workbench-surface"
    @select="select"
    @add="add"
    @duplicate="duplicate"
    @export="exportOne"
    @delete="
      deleteId = $event;
      error = '';
    "
  >
    <EnvironmentEditor
      v-if="draft"
      :key="draft.id"
      v-model="draft"
      :busy="busy"
      :saved="saved"
      :error="error"
      @save="save"
      @dirty="
        saved = false;
        error = '';
      "
    />
    <p v-else class="grid place-items-center p-5 text-sm text-muted-foreground">{{ t("environmentContext.empty") }}</p>
    <Dialog :open="!!deleteItem" @update:open="!$event && !busy && (deleteId = null)">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t("openapi.deleteEnvironmentTitle") }}</DialogTitle>
          <DialogDescription>{{ t("openapi.confirmDeleteEnvironment", { name: deleteItem?.name ?? "" }) }}</DialogDescription>
        </DialogHeader>
        <p v-if="error" role="alert" class="text-destructive">{{ error }}</p>
        <DialogFooter>
          <Button variant="outline" :disabled="busy" @click="deleteId = null">{{ t("common.cancel") }}</Button>
          <Button variant="destructive" :disabled="busy" @click="remove">{{ t("common.delete") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </EnvironmentManager>
</template>
