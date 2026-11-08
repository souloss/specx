<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { FileSearch, GitBranch, Save, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";
import type { RepositoryRefsResponse, SpecificationRefKind } from "@/types/api";

const props = defineProps<{ specificationId: string; serviceId: string }>();
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const busy = ref(false);
const loaded = ref(false);
const error = ref("");
const refKind = ref<SpecificationRefKind>("branch");
const refName = ref("");
const path = ref("");
const confirmRemove = ref(false);
const discovering = ref(false);
const fileSearch = ref("");
const discoveredFiles = ref<Array<{ path: string; byteLength: number; contentHash: string }>>([]);
const repositoryRefs = ref<RepositoryRefsResponse | null>(null);
const refsLoading = ref(false);
const binding = computed(() => workspace.specificationCodeBindingFor(props.specificationId));
const managed = computed(() => Boolean(binding.value?.path));
const repository = computed(() => workspace.serviceRepositoryFor(props.serviceId));
const canSave = computed(() => Boolean(repository.value) && Boolean(refName.value.trim()) && Boolean(path.value.trim()) && !busy.value && !managed.value);
const availableRefs = computed(() => (refKind.value === "branch" ? repositoryRefs.value?.branches : repositoryRefs.value?.tags) ?? []);
const selectedRefAvailable = computed(() => availableRefs.value.includes(refName.value));
const filteredFiles = computed(() => discoveredFiles.value.filter((file) => file.path.toLowerCase().includes(fileSearch.value.trim().toLowerCase())));
let loadRevision = 0;

const refLabel = (kind: SpecificationRefKind) => t(`specification.refKind${kind[0].toUpperCase() + kind.slice(1)}` as "specification.refKindBranch" | "specification.refKindTag" | "specification.refKindCommit");

watch(
  () => [open.value, props.specificationId, props.serviceId] as const,
  async ([value, specificationId, serviceId]) => {
    const revision = ++loadRevision;
    if (!value) return;
    loaded.value = false;
    error.value = "";
    confirmRemove.value = false;
    discoveredFiles.value = [];
    repositoryRefs.value = null;
    refsLoading.value = true;
    fileSearch.value = "";
    busy.value = true;
    try {
      await Promise.all([workspace.loadServiceRepository(serviceId), workspace.loadSpecificationCodeBinding(specificationId)]);
      const linked = workspace.serviceRepositoryFor(serviceId);
      if (linked) repositoryRefs.value = await workspace.discoverRepositoryRefs(linked.repository.id);
      const current = workspace.specificationCodeBindingFor(specificationId);
      refKind.value = current?.refKind === "tag" ? "tag" : "branch";
      const refs = refKind.value === "branch" ? (repositoryRefs.value?.branches ?? []) : (repositoryRefs.value?.tags ?? []);
      refName.value = current && refs.includes(current.refName) ? current.refName : repositoryRefs.value?.defaultBranch || refs[0] || "";
      path.value = current?.path ?? "";
    } catch (cause) {
      if (revision === loadRevision) error.value = cause instanceof Error ? cause.message : t("specification.codeVersionSaveFailed");
    } finally {
      if (revision === loadRevision) {
        loaded.value = true;
        refsLoading.value = false;
        busy.value = false;
      }
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

async function save() {
  if (!canSave.value || !selectedRefAvailable.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.setSpecificationCodeBinding(props.specificationId, refKind.value, refName.value.trim(), path.value.trim());
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.codeVersionSaveFailed");
  } finally {
    busy.value = false;
  }
}

async function discoverFiles() {
  if (!repository.value || !selectedRefAvailable.value || discovering.value || managed.value) return;
  discovering.value = true;
  error.value = "";
  try {
    const result = await workspace.discoverRepositoryFiles(repository.value.repository.id, refKind.value, refName.value.trim());
    discoveredFiles.value = result.files;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.discoveryFailed");
  } finally {
    discovering.value = false;
  }
}

async function remove() {
  if (!binding.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.deleteSpecificationCodeBinding(props.specificationId);
    confirmRemove.value = false;
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.codeVersionSaveFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogScrollContent class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 max-h-[calc(var(--specx-viewport-height)-2rem)] sm:max-w-lg">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle class="flex items-center gap-2 text-base"><GitBranch class="size-4" />{{ t("specification.bindCodeVersion") }}</DialogTitle>
      </DialogHeader>
      <div class="min-h-0 space-y-4 overflow-y-auto overscroll-contain px-5 py-4">
        <p class="text-sm text-muted-foreground">{{ t("specification.codeVersionDescription") }}</p>
        <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>
        <p v-if="!loaded" class="py-8 text-center text-sm text-muted-foreground">{{ t("service.repositoryLoading") }}</p>
        <div v-else-if="!repository" class="rounded-md border border-dashed border-border px-3 py-4 text-sm text-muted-foreground">
          {{ t("specification.codeVersionNotAvailable") }}
        </div>
        <template v-else>
          <div class="rounded-md border border-border bg-muted/20 px-3 py-2.5">
            <span class="eyebrow">{{ t("service.repositoryDetails") }}</span>
            <div class="mt-1 flex min-w-0 items-center gap-2 text-sm font-medium">
              <GitBranch class="size-4 shrink-0 text-primary" />
              <span class="truncate">{{ repository.repository.displayName }}</span>
            </div>
            <code class="mt-1 block truncate text-xs text-muted-foreground">{{ repository.repository.canonicalUrl }}</code>
          </div>
          <p v-if="refsLoading" class="text-sm text-muted-foreground">{{ t("specification.loadingRepositoryRefs") }}</p>
          <p v-else-if="!repositoryRefs || !availableRefs.length" class="text-sm text-destructive">{{ t("specification.noRepositoryRefs") }}</p>
          <div v-else class="grid gap-3 sm:grid-cols-[minmax(0,9rem)_minmax(0,1fr)]">
            <div class="space-y-2">
              <Label for="spec-ref-kind">{{ t("specification.refKind") }}</Label>
              <Select v-model="refKind" :disabled="busy || managed">
                <SelectTrigger id="spec-ref-kind" class="w-full"><SelectValue :placeholder="t('specification.refKind')" /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="branch">{{ t("specification.refKindBranch") }}</SelectItem>
                  <SelectItem value="tag">{{ t("specification.refKindTag") }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-2">
              <Label for="spec-ref-name">{{ t("specification.refName") }}</Label>
              <Select v-model="refName" :disabled="busy || managed">
                <SelectTrigger id="spec-ref-name" class="w-full"><SelectValue :placeholder="t('specification.refName')" /></SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="ref in availableRefs" :key="ref" :value="ref">{{ ref }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-2">
              <Label for="spec-source-path">{{ t("specification.sourcePath") }}</Label
              ><Button variant="outline" size="sm" :disabled="busy || discovering || managed || !refName.trim()" @click="discoverFiles"><FileSearch class="size-3.5" />{{ discovering ? t("specification.discoveringFiles") : t("specification.discoverFiles") }}</Button>
            </div>
            <Input id="spec-source-path" v-model="path" :placeholder="t('specification.sourcePathPlaceholder')" :disabled="busy || Boolean(binding)" />
            <p class="text-xs text-muted-foreground">{{ t("specification.sourcePathDescription") }}</p>
            <div v-if="discoveredFiles.length" class="space-y-2 rounded-md border border-border bg-muted/10 p-2">
              <Input v-model="fileSearch" :placeholder="t('specification.fileSearchPlaceholder')" />
              <p class="text-xs font-medium text-muted-foreground">{{ t("specification.selectMarkdownFile") }}</p>
              <div class="max-h-36 space-y-1 overflow-y-auto">
                <button v-for="file in filteredFiles" :key="file.path" type="button" class="flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-xs hover:bg-accent" :class="path === file.path ? 'bg-primary/10 text-primary' : ''" @click="path = file.path">
                  <span class="truncate">{{ file.path }}</span
                  ><span class="ml-2 shrink-0 text-muted-foreground">{{ file.byteLength }} B</span>
                </button>
                <p v-if="!filteredFiles.length" class="px-2 py-2 text-xs text-muted-foreground">{{ t("specification.noMarkdownFiles") }}</p>
              </div>
            </div>
          </div>
          <p v-if="binding" class="text-xs text-muted-foreground">{{ t("specification.codeVersionBound", { kind: refLabel(binding.refKind), name: binding.refName }) }}</p>
          <p v-if="managed" class="rounded-md border border-primary/30 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">{{ t("specification.managedReadOnlyDescription") }} {{ t("specification.managedUnbindDescription") }}</p>
        </template>
      </div>
      <DialogFooter class="mx-0 mb-0 border-t border-border bg-muted/30 px-5 py-3 sm:flex-row">
        <Button variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button>
        <Button v-if="binding" variant="destructive" :disabled="busy" @click="confirmRemove = true"><Trash2 class="size-4" />{{ t("specification.removeCodeVersion") }}</Button>
        <Button v-if="!managed" :disabled="!canSave" @click="save"><Save class="size-4" />{{ t("common.save") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>

  <Dialog :open="confirmRemove" @update:open="(value) => !busy && (confirmRemove = value)">
    <DialogScrollContent class="sm:max-w-md">
      <DialogHeader
        ><DialogTitle>{{ t("specification.removeCodeVersion") }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("specification.confirmRemoveCodeVersion") }}</p>
      <DialogFooter>
        <Button variant="outline" :disabled="busy" @click="confirmRemove = false">{{ t("common.cancel") }}</Button>
        <Button variant="destructive" :disabled="busy" @click="remove"><Trash2 class="size-4" />{{ t("specification.removeCodeVersion") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
