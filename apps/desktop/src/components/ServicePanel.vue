<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { CircleCheck, Edit3, FileText, GitBranch, History, Plus, RefreshCw, Share2, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import MarkdownPreview from "./specifications/markdown/MarkdownPreview.vue";
import OpenApiModelsPanel from "./specifications/openapi/design/OpenApiModelsPanel.vue";
import type { OpenApiModelChange } from "@/lib/specifications/openapi/contract/openapiModels";
import OpenApiPreview from "./specifications/openapi/OpenApiPreview.vue";
import OpenApiPublicationDialog from "./specifications/openapi/publishing/OpenApiPublicationDialog.vue";
import { isTauriRuntime } from "@/lib/backend/runtime";
import OpenApiProviderPanel from "./specifications/openapi/OpenApiProviderPanel.vue";
import OpenApiOperationCreateDialog from "./specifications/openapi/design/OpenApiOperationCreateDialog.vue";
import type { OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import SpecificationEditor from "./specifications/SpecificationEditor.vue";
import SpecificationHistory from "./specifications/SpecificationHistory.vue";
import CodeBindingDialog from "./CodeBindingDialog.vue";
import SpecificationCodeBindingDialog from "./specifications/SpecificationCodeBindingDialog.vue";
import TagBadge from "./TagBadge.vue";
import { formatDate, groupPath } from "../lib/core";
import { i18n, translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Catalog, PanelTab } from "../types/domain";

const props = defineProps<{ catalog: Catalog; serviceId: string; tab: PanelTab }>();
const emit = defineEmits<{ openSpecification: [id: string]; newSpecification: [] }>();
const workspace = useWorkspaceStore();
const service = computed(() => props.catalog.services.find((item) => item.id === props.serviceId));
const activeSpecification = computed(() => (props.tab.specificationId ? props.catalog.specifications.find((item) => item.id === props.tab.specificationId) : undefined));
const modelTarget = computed(() => Boolean(props.tab.modelRef || props.tab.modelsDirectory));
const activeProvider = computed(() => (props.tab.providerKind === "openapi" || activeSpecification.value?.kind === "openapi" ? workspace.openApiProviderFor(props.serviceId) : null));
const specifications = computed(() => props.catalog.specifications.filter((item) => item.serviceId === props.serviceId));
const tags = computed(() => service.value?.tagIds.map((id) => props.catalog.tags.find((tag) => tag.id === id)).filter(Boolean) ?? []);
const t = translate;
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
const editing = ref(false);
const createOperationOpen = ref(false);
const historyOpen = ref(false);
const publicationOpen = ref(false);
const canPublish = computed(() => !isTauriRuntime() && workspace.workspaces.some((item) => item.id === props.catalog.workspace.id && ["owner", "admin"].includes(item.role)));
const deleteOpen = ref(false);
const deleting = ref(false);
const error = ref("");
const codeBindingsOpen = ref(false);
const specificationCodeBindingOpen = ref(false);
const sourceMetadataLoading = ref(false);
const specificationCodeBinding = computed(() => (activeSpecification.value ? workspace.specificationCodeBindingFor(activeSpecification.value.id) : null));
const repositoryManaged = computed(() => Boolean(specificationCodeBinding.value?.path));
const specificationSync = computed(() => (activeSpecification.value ? workspace.specificationSyncFor(activeSpecification.value.id) : null));
const syncing = ref(false);
const repositoryLoading = ref(false);
const repositoryLoadError = ref("");
const serviceRepository = computed(() => workspace.serviceRepositoryFor(props.serviceId));
let sourceLoadRevision = 0;
let repositoryLoadRevision = 0;

function specificationKindLabel(kind: string) {
  const normalized = kind.toLocaleLowerCase();
  if (normalized === "markdown") return t("specification.kinds.markdown");
  if (normalized === "openapi") return t("specification.kinds.openapi");
  return kind;
}

watch(
  () => [props.serviceId, props.catalog] as const,
  async ([serviceId]) => {
    const revision = ++repositoryLoadRevision;
    repositoryLoading.value = Boolean(serviceId);
    repositoryLoadError.value = "";
    if (!serviceId) return;
    try {
      await workspace.loadServiceRepository(serviceId);
    } catch (cause) {
      if (revision === repositoryLoadRevision) repositoryLoadError.value = cause instanceof Error ? cause.message : t("service.repositoryLoadFailed");
    } finally {
      if (revision === repositoryLoadRevision) repositoryLoading.value = false;
    }
  },
  { immediate: true },
);

watch(
  () => [props.tab.kind, props.tab.providerKind, props.serviceId, activeSpecification.value?.kind, props.catalog] as const,
  ([kind, providerKind, serviceId, specificationKind]) => {
    if (((kind === "provider" && providerKind === "openapi") || (kind === "specification" && specificationKind === "openapi")) && !workspace.openApiProviderFor(serviceId)) void workspace.loadOpenApiProvider(serviceId);
  },
  { immediate: true },
);

watch(
  () => [activeSpecification.value?.id, props.catalog] as const,
  async ([specificationId]) => {
    const revision = ++sourceLoadRevision;
    editing.value = false;
    createOperationOpen.value = false;
    historyOpen.value = false;
    publicationOpen.value = false;
    deleteOpen.value = false;
    error.value = "";
    sourceMetadataLoading.value = Boolean(specificationId);
    if (!specificationId) return;
    try {
      const binding = await workspace.loadSpecificationCodeBinding(specificationId);
      if (binding?.path) await workspace.loadSpecificationSyncStatus(specificationId);
    } catch {
      // The detail view remains usable when optional source metadata is unavailable.
    } finally {
      if (revision === sourceLoadRevision) {
        sourceMetadataLoading.value = false;
      }
    }
  },
  { immediate: true },
);

async function removeSpecification() {
  if (!activeSpecification.value || deleting.value) return;
  deleting.value = true;
  error.value = "";
  try {
    const serviceId = activeSpecification.value.serviceId;
    await workspace.deleteSpecification(activeSpecification.value.id);
    deleteOpen.value = false;
    workspace.openOverview(serviceId);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.deleteFailed");
  } finally {
    deleting.value = false;
  }
}

async function synchronizeSpecification() {
  const specification = activeSpecification.value;
  if (!specification || !repositoryManaged.value || syncing.value || sourceMetadataLoading.value) return;
  syncing.value = true;
  error.value = "";
  try {
    await workspace.syncSpecification(specification.id);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.syncFailed");
  } finally {
    syncing.value = false;
  }
}

function navigateModel(reference?: string) {
  if (activeSpecification.value) workspace.openOpenApiModel(activeSpecification.value.id, reference);
}
function navigateOperation(key: string, title: string) {
  if (activeSpecification.value) workspace.openSpecification(activeSpecification.value.id, key, title);
}
async function saveOpenApiModel(content: string, change?: OpenApiModelChange) {
  const specification = activeSpecification.value;
  if (!specification || specification.kind !== "openapi" || sourceMetadataLoading.value || repositoryManaged.value) return false;
  return workspace.saveOpenApiDocument(specification.id, content, change);
}

async function saveOpenApiOperation(content: string, change?: OpenApiOperationChange) {
  const specification = activeSpecification.value;
  if (!specification || specification.kind !== "openapi" || repositoryManaged.value) return false;
  return workspace.saveOpenApiOperation(specification.id, content, change);
}
</script>

<template>
  <div v-if="!service" class="panel-empty">
    <h2>{{ t("service.unavailable") }}</h2>
    <p>{{ t("service.unavailableDescription") }}</p>
  </div>
  <div v-else-if="props.tab.kind === 'provider' && props.tab.providerKind === 'openapi'" class="spec-panel h-full min-h-0 overflow-auto pr-1">
    <OpenApiProviderPanel v-if="activeProvider" :service-id="props.serviceId" :provider="activeProvider" />
    <div v-else class="panel-empty">
      <span class="eyebrow">{{ t("specification.kinds.openapi") }}</span>
      <h2>{{ t("common.loading") }}</h2>
    </div>
  </div>
  <div v-else-if="activeSpecification" class="spec-panel h-full min-h-0 overflow-auto pr-1" :class="{ 'spec-panel-openapi-operation': activeSpecification.kind === 'openapi' && Boolean(props.tab.operationKey || modelTarget) }">
    <header v-if="!editing && !props.tab.operationKey && !modelTarget" class="spec-toolbar">
      <div v-if="!props.tab.operationKey">
        <span class="eyebrow">{{ t("service.specificationHeading") }}</span>
        <h2>{{ props.tab.operationKey ? props.tab.title : activeSpecification.title }}</h2>
        <span class="muted">{{ props.tab.operationKey ? `${activeSpecification.title} · ` : `${service.name} · ` }}{{ t("service.updatedAt", { date: formatDate(activeSpecification.updatedAt, locale) }) }} · {{ t("specification.currentVersion", { version: activeSpecification.version }) }}</span>
      </div>
      <div v-if="!editing" class="flex flex-wrap items-center gap-2">
        <span v-if="sourceMetadataLoading" class="text-xs text-muted-foreground">{{ t("common.loading") }}</span>
        <Button v-if="repositoryManaged" size="sm" :disabled="sourceMetadataLoading || syncing" @click="synchronizeSpecification"><RefreshCw class="h-3.5 w-3.5" :class="syncing ? 'animate-spin' : undefined" />{{ syncing ? t("specification.syncing") : t("specification.syncNow") }}</Button>
        <Button v-if="!repositoryManaged" size="sm" variant="outline" :disabled="sourceMetadataLoading" @click="specificationCodeBindingOpen = true"><GitBranch class="h-3.5 w-3.5" />{{ t("specification.bindCodeVersion") }}</Button>
        <Button v-if="!repositoryManaged && activeSpecification.kind === 'openapi'" size="sm" :disabled="sourceMetadataLoading" @click="createOperationOpen = true"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiCreateOperation") }}</Button>
        <Button v-if="!repositoryManaged" size="sm" :disabled="sourceMetadataLoading" @click="editing = true"><Edit3 class="h-3.5 w-3.5" />{{ t("service.editSpecification") }}</Button>
        <Button v-if="activeSpecification.kind === 'openapi' && canPublish" size="sm" variant="outline" @click="publicationOpen = true"><Share2 class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.manage") }}</Button>
        <Button size="sm" variant="outline" :disabled="sourceMetadataLoading" @click="historyOpen = true"><History class="h-3.5 w-3.5" />{{ t("service.specificationHistory") }}</Button>
        <Button v-if="!repositoryManaged" size="sm" variant="destructive" :disabled="sourceMetadataLoading" @click="deleteOpen = true"><Trash2 class="h-3.5 w-3.5" />{{ t("service.deleteSpecification") }}</Button>
      </div>
    </header>
    <div v-if="repositoryManaged" class="mb-4 flex items-start gap-3 rounded-md border border-primary/30 bg-primary/5 p-3 text-sm">
      <GitBranch class="mt-0.5 h-4 w-4 shrink-0 text-primary" />
      <div>
        <strong class="block">{{ t("specification.managedReadOnly") }}</strong>
        <span class="text-xs text-muted-foreground">{{ t("specification.managedReadOnlyDescription") }}</span>
        <span v-if="specificationCodeBinding" class="mt-1 block truncate font-mono text-xs text-muted-foreground">{{ specificationCodeBinding.path }} · {{ specificationCodeBinding.refName }}</span>
        <div v-if="specificationSync" class="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-primary/15 pt-2 text-xs text-muted-foreground" role="status">
          <span class="font-medium text-foreground">{{ specificationSync.changed ? t("specification.syncUpdated", { commit: specificationSync.resolvedCommit.slice(0, 7) }) : t("specification.syncUnchanged", { commit: specificationSync.resolvedCommit.slice(0, 7) }) }}</span>
          <span :title="specificationSync.manifestHash">{{ t("specification.syncManifest", { hash: specificationSync.manifestHash.slice(0, 15) }) }}</span>
          <span>{{ t("specification.syncLastRun", { date: formatDate(specificationSync.syncedAt, locale) }) }}</span>
        </div>
      </div>
    </div>
    <p v-if="error" role="alert" class="mb-4 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>
    <SpecificationEditor v-if="editing && !repositoryManaged && !props.tab.operationKey" :specification="activeSpecification" @close="editing = false" />
    <OpenApiModelsPanel
      v-else-if="activeSpecification.kind === 'openapi' && modelTarget"
      :key="`${activeSpecification.id}:${props.tab.modelRef ?? 'models'}`"
      :content="activeSpecification.content"
      :model-ref="props.tab.modelRef"
      :specification-title="activeSpecification.title"
      :editable="!sourceMetadataLoading && !repositoryManaged"
      :draft-scope="JSON.stringify([props.catalog.workspace.id, activeSpecification.id])"
      :save-document="saveOpenApiModel"
      :navigate-model="navigateModel"
      :navigate-operation="navigateOperation"
    />
    <div v-else class="spec-layout" :class="{ 'spec-layout-openapi': activeSpecification.kind === 'openapi', 'spec-layout-openapi-operation': activeSpecification.kind === 'openapi' && Boolean(props.tab.operationKey || modelTarget) }">
      <OpenApiPreview
        v-if="activeSpecification.kind === 'openapi'"
        :key="activeSpecification.id"
        :content="activeSpecification.content"
        :focus-operation-key="props.tab.operationKey"
        :service-id="props.serviceId"
        :environments="activeProvider?.environments ?? []"
        :default-environment-id="activeProvider?.settings.defaultEnvironmentId"
        :default-server-url="activeProvider?.settings.defaultServerUrl"
        :show-operation-id="activeProvider?.settings.showOperationId ?? true"
        :schema-display="activeProvider?.settings.schemaDisplay ?? 'summary'"
        :specification-title="activeSpecification.title"
        :specification-version="activeSpecification.version"
        :design-draft-scope="JSON.stringify([props.catalog.workspace.id, activeSpecification.id])"
        :editable="!sourceMetadataLoading && !repositoryManaged"
        :save-operation="saveOpenApiOperation"
        :navigate-model="navigateModel"
      />
      <MarkdownPreview v-else :content="activeSpecification.content" />
      <aside v-if="activeSpecification.kind !== 'openapi'" class="spec-meta">
        <span class="eyebrow">{{ t("service.serviceLabel") }}</span
        ><strong>{{ service.name }}</strong
        ><span>{{ service.slug }}</span>
        <span class="mt-3 text-xs">{{ t("specification.currentVersion", { version: activeSpecification.version }) }}</span>
      </aside>
    </div>
    <OpenApiPublicationDialog v-if="publicationOpen && canPublish && activeSpecification.kind === 'openapi'" :key="activeSpecification.id" :specification="activeSpecification" :workspace-id="catalog.workspace.id" :environments="activeProvider?.environments" @close="publicationOpen = false" />
    <SpecificationHistory v-model:open="historyOpen" :specification="activeSpecification" :managed="repositoryManaged" />
    <OpenApiOperationCreateDialog v-if="activeSpecification.kind === 'openapi'" v-model:open="createOperationOpen" :content="activeSpecification.content" :editable="!sourceMetadataLoading && !repositoryManaged" :save-operation="saveOpenApiOperation" />
    <SpecificationCodeBindingDialog v-model:open="specificationCodeBindingOpen" :specification-id="activeSpecification.id" :service-id="activeSpecification.serviceId" />
    <Dialog v-model:open="deleteOpen">
      <DialogContent>
        <DialogHeader
          ><DialogTitle>{{ t("specification.deleteTitle") }}</DialogTitle></DialogHeader
        >
        <p class="text-sm text-muted-foreground">{{ t("specification.confirmDelete", { name: activeSpecification.title }) }}</p>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <DialogFooter
          ><Button variant="outline" :disabled="deleting" @click="deleteOpen = false">{{ t("common.cancel") }}</Button
          ><Button variant="destructive" :disabled="deleting" @click="removeSpecification"><Trash2 class="h-3.5 w-3.5" />{{ deleting ? t("app.deleting") : t("common.delete") }}</Button></DialogFooter
        >
      </DialogContent>
    </Dialog>
  </div>
  <div v-else class="overview-panel h-full overflow-auto pr-1">
    <header class="overview-hero">
      <div>
        <span class="eyebrow">{{ t("service.overview") }}</span>
        <h2>{{ service.name }}</h2>
        <p>{{ service.description || t("service.noDescription") }}</p>
        <div class="service-identity">
          <span
            ><b>{{ t("service.slug") }}</b
            >{{ service.slug }}</span
          ><span
            ><b>{{ t("service.directory") }}</b
            >{{ groupPath(catalog.groups, service.groupId).join(" / ") || t("common.ungrouped") }}</span
          >
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <span v-if="repositoryLoading" class="text-xs text-muted-foreground">{{ t("service.repositoryLoading") }}</span>
        <span v-else-if="repositoryLoadError" class="max-w-48 truncate text-xs text-destructive">{{ repositoryLoadError }}</span>
        <Button v-else-if="serviceRepository" size="sm" variant="outline" class="max-w-64" :title="`${serviceRepository.repository.displayName} · ${serviceRepository.repository.provider}`" @click="codeBindingsOpen = true">
          <CircleCheck class="size-3.5 shrink-0 text-emerald-600" aria-hidden="true" />
          <span class="min-w-0 truncate">{{ serviceRepository.repository.displayName }}</span>
          <span class="shrink-0 text-xs text-muted-foreground">{{ serviceRepository.repository.provider }}</span>
        </Button>
        <Button v-else size="sm" variant="outline" @click="codeBindingsOpen = true"><GitBranch class="size-3.5" />{{ t("service.codeBindings") }}</Button>
        <Button size="sm" @click="emit('newSpecification')"><Plus class="size-3.5" />{{ t("service.createSpecification") }}</Button>
      </div>
    </header>
    <section class="content-section">
      <div class="section-heading">
        <div>
          <span class="eyebrow">{{ t("service.specifications") }}</span>
          <h3>
            {{ t("service.specifications") }} <small>{{ specifications.length }}</small>
          </h3>
        </div>
        <button class="text-button" type="button" @click="emit('newSpecification')"><Plus :size="14" />{{ t("service.create") }}</button>
      </div>
      <div v-if="!specifications.length" class="section-empty">
        <FileText :size="22" /><strong>{{ t("service.noSpecifications") }}</strong
        ><span>{{ t("service.addFirstSpecification") }}</span>
      </div>
      <div v-else class="spec-list">
        <button v-for="specification in specifications" :key="specification.id" class="spec-row" @click="emit('openSpecification', specification.id)">
          <FileText :size="17" /><span
            ><strong>{{ specification.title }}</strong
            ><small>{{ specificationKindLabel(specification.kind) }} · {{ t("service.updatedAt", { date: formatDate(specification.updatedAt, locale) }) }}</small></span
          >
        </button>
      </div>
    </section>
    <section class="content-section service-facts">
      <span class="eyebrow">{{ t("service.details") }}</span>
      <h3>{{ t("service.details") }}</h3>
      <div class="facts-list">
        <div>
          <span>{{ t("service.lastUpdated") }}</span
          ><strong>{{ formatDate(service.updatedAt, locale) }}</strong>
        </div>
      </div>
      <div class="tag-list">
        <TagBadge v-for="tag in tags" :key="tag!.id" :tag="tag!" />
        <span v-if="!tags.length" class="muted">{{ t("service.noTags") }}</span>
      </div>
    </section>
    <CodeBindingDialog v-model:open="codeBindingsOpen" :service-id="serviceId" />
  </div>
</template>
