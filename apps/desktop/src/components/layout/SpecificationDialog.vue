<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowLeft, ArrowRight, Check, FileText, Grid2X2, List, Search } from "@lucide/vue";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { useWorkspaceStore } from "@/stores/workspace";
import { specxApi } from "@/lib/backend/client";
import { createId } from "@/lib/core";
import { SPECIFICATION_TYPES, type SpecificationFieldDefinition, type SpecificationTypeDefinition } from "@/lib/specifications/registry";
import type { SpecificationKind } from "@/types/domain";
import type { RepositoryFileResponse, RepositoryRefsResponse, SpecificationRefKind } from "@/types/api";
import { translate, type MessageKey } from "@/i18n";

const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const step = ref(1);
const query = ref("");
const selectedKind = ref<SpecificationKind>("markdown");
const values = ref<Record<string, string>>({});
const error = ref("");
const busy = ref(false);
const serviceId = ref("");
const viewMode = ref<"grid" | "list">("grid");
const categoryFilter = ref("all");
const sourceMode = ref<"specx" | "repository">("specx");
const refKind = ref<SpecificationRefKind>("branch");
const refName = ref("");
const sourcePath = ref("");
const repositoryLoading = ref(false);
const repositoryRefsLoading = ref(false);
const repositoryRefs = ref<RepositoryRefsResponse | null>(null);
const selectedType = computed(() => SPECIFICATION_TYPES.find((item) => item.kind === selectedKind.value) ?? SPECIFICATION_TYPES[0]!);
const currentStep = computed(() => selectedType.value.steps[step.value - 1] ?? selectedType.value.steps[0]!);
const categories = computed(() => {
  const registered = new Map(SPECIFICATION_TYPES.map((item) => [item.category, item.categoryKey]));
  return [{ id: "all", labelKey: "specification.categories.all" as MessageKey }, ...[...registered].map(([id, labelKey]) => ({ id, labelKey }))];
});
const visibleTypes = computed(() => {
  const normalized = query.value.trim().toLocaleLowerCase();
  return SPECIFICATION_TYPES.filter((item) => item.enabled && (categoryFilter.value === "all" || item.category === categoryFilter.value) && (!normalized || [t(item.titleKey), t(item.descriptionKey), t(item.keywordsKey)].join(" ").toLocaleLowerCase().includes(normalized)));
});
const serviceRepository = computed(() => workspace.serviceRepositoryFor(serviceId.value));
const availableRefs = computed(() => {
  if (!repositoryRefs.value) return [];
  return refKind.value === "branch" ? repositoryRefs.value.branches : repositoryRefs.value.tags;
});
const validSourcePath = computed(() => {
  const value = sourcePath.value.trim();
  if (!value || value.startsWith("/") || value.startsWith("\\") || value.endsWith("/") || value.includes("\\") || value.includes("..")) return false;
  if (value.split("/").some((part) => !part || part === ".")) return false;
  if ([...value].some((character) => "~^:?*[".includes(character))) return false;
  const lower = value.toLocaleLowerCase();
  return lower.endsWith(".md") || lower.endsWith(".markdown");
});
const canContinue = computed(() => {
  if (currentStep.value.kind === "type-picker") return Boolean(selectedKind.value);
  const fieldsReady = currentStep.value.fields?.every((field) => !field.required || fieldValue(field).trim()) !== false;
  if (currentStep.value.kind !== "fields" || sourceMode.value === "specx") return fieldsReady;
  return Boolean(serviceRepository.value) && Boolean(repositoryRefs.value) && availableRefs.value.includes(refName.value) && validSourcePath.value;
});

function initialValues(type: SpecificationTypeDefinition) {
  const next: Record<string, string> = {};
  for (const step of type.steps) {
    for (const field of step.fields ?? []) {
      next[field.id] = field.defaultValueKey ? t(field.defaultValueKey) : "";
    }
  }
  return next;
}

watch(open, (value) => {
  if (!value) return;
  step.value = 1;
  query.value = "";
  viewMode.value = "grid";
  categoryFilter.value = "all";
  selectedKind.value = "markdown";
  values.value = initialValues(selectedType.value);
  sourceMode.value = "specx";
  refKind.value = "branch";
  refName.value = "";
  sourcePath.value = "";
  repositoryRefs.value = null;
  repositoryRefsLoading.value = false;
  error.value = "";
  serviceId.value = workspace.activeServiceId ?? "";
});
watch(
  () => [sourceMode.value, serviceId.value] as const,
  async ([mode, currentServiceId]) => {
    if (mode !== "repository" || !currentServiceId) return;
    repositoryLoading.value = true;
    repositoryRefsLoading.value = true;
    repositoryRefs.value = null;
    refName.value = "";
    error.value = "";
    try {
      await workspace.loadServiceRepository(currentServiceId);
      if (serviceRepository.value) {
        const refs = await workspace.discoverRepositoryRefs(serviceRepository.value.repository.id);
        repositoryRefs.value = refs;
        refKind.value = "branch";
        refName.value = refs.defaultBranch || refs.branches[0] || refs.tags[0] || "";
        if (!refName.value) error.value = t("specification.noRepositoryRefs");
      }
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : t("specification.sourceRepositoryRequired");
    } finally {
      repositoryLoading.value = false;
      repositoryRefsLoading.value = false;
    }
  },
);
watch(
  [refKind, availableRefs],
  () => {
    if (availableRefs.value.length && !availableRefs.value.includes(refName.value)) refName.value = availableRefs.value[0]!;
  },
  { deep: true },
);
function selectType(kind: SpecificationKind) {
  selectedKind.value = kind;
  const type = SPECIFICATION_TYPES.find((item) => item.kind === kind);
  if (type) values.value = { ...initialValues(type), ...values.value };
}
function fieldValue(field: Pick<SpecificationFieldDefinition, "id">) {
  return values.value[field.id] ?? "";
}
function updateField(field: SpecificationFieldDefinition, event: Event) {
  const value = (event.target as HTMLInputElement | HTMLTextAreaElement).value;
  values.value = { ...values.value, [field.id]: value };
}
function next() {
  if (!canContinue.value || step.value >= selectedType.value.steps.length) return;
  step.value += 1;
}
function previous() {
  if (step.value > 1) step.value -= 1;
}
async function save() {
  if (!workspace.catalog || !serviceId.value || busy.value || !canContinue.value) return;
  busy.value = true;
  error.value = "";
  try {
    const id = createId("spec");
    const request = selectedType.value.createRequest({ id, serviceId: serviceId.value, values: sourceMode.value === "repository" ? { ...values.value, title: "", content: "" } : values.value });
    if (sourceMode.value === "repository") {
      const discovery = await workspace.discoverRepositoryFiles(serviceRepository.value!.repository.id, refKind.value, refName.value);
      if (!discovery.files.some((file: RepositoryFileResponse) => file.path === sourcePath.value.trim())) {
        throw new Error(t("specification.sourcePathNotFound"));
      }
      request.source = { path: sourcePath.value.trim(), refKind: refKind.value, refName: refName.value.trim() };
    }
    await specxApi.createSpecification(workspace.catalog.workspace.id, request);
    await workspace.refresh();
    workspace.openSpecification(id);
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.createFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogContent class="grid-rows-[auto_minmax(0,1fr)] sm:max-w-4xl" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()">
      <DialogHeader>
        <DialogTitle>{{ t("specification.createTitle") }}</DialogTitle>
      </DialogHeader>
      <form class="flex min-h-0 flex-col gap-4 overflow-hidden" @submit.prevent="step === selectedType.steps.length ? save() : next()">
        <section v-if="currentStep.kind === 'type-picker'" class="flex min-h-0 flex-1 flex-col gap-3">
          <div class="flex shrink-0 items-center gap-2">
            <div class="relative min-w-0 flex-1">
              <Search class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" /><input
                v-model="query"
                class="settings-input specification-type-search-input h-9"
                :aria-label="t('specification.searchTypes')"
                :placeholder="t('specification.searchTypes')"
                autocomplete="off"
                autocapitalize="off"
                autocorrect="off"
                spellcheck="false"
              />
            </div>
            <div class="flex shrink-0 items-center rounded-md border border-border p-0.5">
              <button type="button" class="rounded p-1.5" :class="viewMode === 'grid' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent'" :aria-label="t('specification.gridView')" :aria-pressed="viewMode === 'grid'" @click="viewMode = 'grid'"><Grid2X2 class="h-4 w-4" /></button>
              <button type="button" class="rounded p-1.5" :class="viewMode === 'list' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent'" :aria-label="t('specification.listView')" :aria-pressed="viewMode === 'list'" @click="viewMode = 'list'"><List class="h-4 w-4" /></button>
            </div>
          </div>
          <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border border-border sm:flex-row">
            <nav class="flex shrink-0 gap-1 overflow-x-auto border-b border-border bg-muted/20 p-2 sm:w-40 sm:flex-col sm:overflow-y-auto sm:border-r sm:border-b-0" :aria-label="t('specification.categoryNavigation')">
              <button
                v-for="category in categories"
                :key="category.id"
                type="button"
                class="shrink-0 rounded px-3 py-2 text-left text-xs"
                :class="categoryFilter === category.id ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground'"
                :aria-current="categoryFilter === category.id ? 'page' : undefined"
                @click="categoryFilter = category.id"
              >
                {{ t(category.labelKey) }}
              </button>
            </nav>
            <div class="min-h-0 flex-1 overflow-y-auto p-3">
              <div class="gap-2" :class="viewMode === 'grid' ? 'grid sm:grid-cols-2' : 'flex flex-col'">
                <button v-for="item in visibleTypes" :key="item.kind" type="button" class="flex items-start gap-3 rounded-md border p-3 text-left transition-colors hover:bg-accent" :class="selectedKind === item.kind ? 'border-primary bg-accent' : 'border-border'" @click="selectType(item.kind)">
                  <FileText class="mt-0.5 h-5 w-5 shrink-0 text-primary" />
                  <span class="min-w-0"
                    ><strong class="block text-sm font-medium">{{ t(item.titleKey) }}</strong
                    ><span class="mt-1 block text-xs leading-relaxed text-muted-foreground">{{ t(item.descriptionKey) }}</span></span
                  >
                </button>
              </div>
              <p v-if="!visibleTypes.length" class="py-12 text-center text-sm text-muted-foreground">{{ t("specification.noTypes") }}</p>
            </div>
          </div>
        </section>
        <section v-else-if="currentStep.kind === 'fields'" class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
          <div class="rounded-md border border-border bg-muted/20 p-3">
            <div class="text-sm font-medium">{{ t(selectedType.titleKey) }}</div>
            <div class="text-xs text-muted-foreground">{{ t(selectedType.descriptionKey) }}</div>
          </div>
          <fieldset class="space-y-3">
            <legend class="text-sm font-medium">{{ t("specification.sourceMode") }}</legend>
            <p class="text-xs text-muted-foreground">{{ t("specification.sourceModeDescription") }}</p>
            <div class="grid gap-2 sm:grid-cols-2">
              <button type="button" class="rounded-md border p-3 text-left transition-colors hover:bg-accent" :class="sourceMode === 'specx' ? 'border-primary bg-accent' : 'border-border'" :aria-pressed="sourceMode === 'specx'" :disabled="busy" @click="sourceMode = 'specx'">
                <strong class="block text-sm">{{ t("specification.sourceModeSpecx") }}</strong>
                <span class="mt-1 block text-xs leading-relaxed text-muted-foreground">{{ t("specification.sourceModeSpecxDescription") }}</span>
              </button>
              <button type="button" class="rounded-md border p-3 text-left transition-colors hover:bg-accent" :class="sourceMode === 'repository' ? 'border-primary bg-accent' : 'border-border'" :aria-pressed="sourceMode === 'repository'" :disabled="busy" @click="sourceMode = 'repository'">
                <strong class="block text-sm">{{ t("specification.sourceModeRepository") }}</strong>
                <span class="mt-1 block text-xs leading-relaxed text-muted-foreground">{{ t("specification.sourceModeRepositoryDescription") }}</span>
              </button>
            </div>
          </fieldset>
          <div v-if="sourceMode === 'repository'" class="space-y-4 rounded-md border border-primary/30 bg-primary/5 p-3">
            <p class="text-sm text-muted-foreground">{{ t("specification.sourceRepositoryDescription") }}</p>
            <p v-if="repositoryLoading" class="text-sm text-muted-foreground">{{ t("service.repositoryLoading") }}</p>
            <p v-else-if="!serviceRepository" class="text-sm text-destructive">{{ t("specification.sourceRepositoryRequired") }}</p>
            <template v-else>
              <div class="rounded-md border border-border bg-background px-3 py-2 text-sm">
                <strong class="block truncate">{{ serviceRepository.repository.displayName }}</strong>
                <code class="mt-1 block truncate text-xs text-muted-foreground">{{ serviceRepository.repository.canonicalUrl }}</code>
              </div>
              <p v-if="repositoryRefsLoading" class="text-sm text-muted-foreground">{{ t("specification.loadingRepositoryRefs") }}</p>
              <p v-else-if="!repositoryRefs || !availableRefs.length" class="text-sm text-destructive">{{ t("specification.noRepositoryRefs") }}</p>
              <div v-else class="grid gap-3 sm:grid-cols-[minmax(0,9rem)_minmax(0,1fr)]">
                <label class="settings-field"
                  >{{ t("specification.refKind")
                  }}<Select v-model="refKind">
                    <SelectTrigger class="settings-input h-9 w-full"><SelectValue :placeholder="t('specification.refKind')" /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="branch">{{ t("specification.refKindBranch") }}</SelectItem>
                      <SelectItem value="tag">{{ t("specification.refKindTag") }}</SelectItem>
                    </SelectContent>
                  </Select></label
                >
                <label class="settings-field"
                  >{{ t("specification.refName")
                  }}<Select v-model="refName">
                    <SelectTrigger class="settings-input h-9 w-full"><SelectValue :placeholder="t('specification.refName')" /></SelectTrigger>
                    <SelectContent>
                      <SelectItem v-for="ref in availableRefs" :key="ref" :value="ref">{{ ref }}</SelectItem>
                    </SelectContent>
                  </Select></label
                >
              </div>
              <label class="settings-field"
                >{{ t("specification.sourcePath")
                }}<input v-model="sourcePath" class="settings-input" :class="{ 'border-destructive': sourcePath.trim() && !validSourcePath }" :aria-invalid="Boolean(sourcePath.trim() && !validSourcePath)" :placeholder="t('specification.sourcePathPlaceholder')" autocomplete="off" /><span
                  class="text-xs font-normal text-muted-foreground"
                  >{{ t("specification.sourcePathDescription") }}</span
                ><span v-if="sourcePath.trim() && !validSourcePath" class="text-xs font-normal text-destructive">{{ t("specification.sourcePathInvalid") }}</span></label
              >
            </template>
          </div>
          <template v-if="sourceMode === 'specx'">
            <label v-for="field in currentStep.fields ?? []" :key="field.id" class="settings-field">
              {{ t(field.labelKey) }}
              <input v-if="field.control === 'text'" :value="fieldValue(field)" class="settings-input" :required="field.required" autofocus @input="updateField(field, $event)" />
              <textarea v-else :value="fieldValue(field)" class="settings-input min-h-56 resize-y font-mono" :rows="field.rows ?? 8" :required="field.required" @input="updateField(field, $event)" />
            </label>
          </template>
        </section>
        <section v-else class="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
          <div class="rounded-md border border-border p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium"><Check class="h-4 w-4 text-primary" />{{ t("specification.ready") }}</div>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
              <dt class="text-muted-foreground">{{ t("specification.type") }}</dt>
              <dd>{{ t(selectedType.titleKey) }}</dd>
              <dt class="text-muted-foreground">{{ t("specification.sourceMode") }}</dt>
              <dd>{{ t(sourceMode === "repository" ? "specification.sourceModeRepository" : "specification.sourceModeSpecx") }}</dd>
              <template v-if="sourceMode === 'repository'">
                <dt class="text-muted-foreground">{{ t("specification.sourcePath") }}</dt>
                <dd class="truncate">{{ sourcePath }}</dd>
                <dt class="text-muted-foreground">{{ t("specification.refName") }}</dt>
                <dd class="truncate">{{ refName }}</dd>
              </template>
              <template v-if="sourceMode === 'specx'">
                <template v-for="summaryField in currentStep.summaryFields ?? []" :key="summaryField.id">
                  <dt class="text-muted-foreground">{{ t(summaryField.labelKey) }}</dt>
                  <dd class="truncate">{{ fieldValue(summaryField) }}</dd>
                </template>
              </template>
            </dl>
          </div>
          <p class="text-xs text-muted-foreground">{{ t(sourceMode === "repository" ? "specification.confirmRepositoryDescription" : "specification.confirmDescription") }}</p>
        </section>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <DialogFooter>
          <Button type="button" variant="outline" :disabled="busy" @click="step === 1 ? (open = false) : previous()"><ArrowLeft v-if="step > 1" class="h-3.5 w-3.5" />{{ step === 1 ? t("common.cancel") : t("common.back") }}</Button>
          <Button v-if="step < selectedType.steps.length" type="submit" :disabled="!canContinue"><ArrowRight class="h-3.5 w-3.5" />{{ t("common.next") }}</Button>
          <Button v-else type="submit" :disabled="busy || !canContinue">{{ busy ? t("common.creating") : t("specification.create") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.specification-type-search-input {
  padding-left: 2.25rem;
}
</style>
