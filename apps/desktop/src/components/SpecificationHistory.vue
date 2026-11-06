<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Clock3, Eye, RotateCcw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import MarkdownPreview from "./MarkdownPreview.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Specification } from "@/types/domain";
import { formatDate } from "@/lib/core";
import { i18n, translate } from "@/i18n";

const props = defineProps<{ specification: Specification }>();
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const selectedVersion = ref<number | null>(null);
const restoreVersion = ref<number | null>(null);
const error = ref("");
const busy = ref(false);
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
const revisions = computed(() => workspace.specificationRevisions(props.specification.id));
const selected = computed(() => revisions.value.find((item) => item.version === selectedVersion.value));
const loading = computed(() => workspace.specificationRevisionsLoading(props.specification.id));
const hasDraft = computed(() => workspace.hasUnsavedSpecification(props.specification.id));

watch(open, async (value) => {
  if (!value) return;
  error.value = "";
  try {
    const loaded = await workspace.loadSpecificationRevisions(props.specification.id);
    selectedVersion.value = loaded.find((revision) => revision.version === props.specification.version)?.version ?? loaded[0]?.version ?? null;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.historyLoadFailed");
  }
});

async function restore() {
  if (!restoreVersion.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.restoreSpecificationRevision(props.specification.id, restoreVersion.value);
    selectedVersion.value = null;
    restoreVersion.value = null;
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.restoreFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-5xl sm:grid-rows-[auto_minmax(0,1fr)_auto]">
      <DialogHeader
        ><DialogTitle class="flex items-center gap-2"><Clock3 class="h-4 w-4" />{{ t("specification.revisionHistory") }}</DialogTitle></DialogHeader
      >
      <div class="grid min-h-0 gap-4 sm:grid-cols-[220px_minmax(0,1fr)]">
        <nav class="min-h-0 overflow-y-auto overscroll-contain rounded-md border border-border p-1" :aria-label="t('specification.revisionHistory')">
          <p v-if="loading" class="p-3 text-xs text-muted-foreground">{{ t("common.loading") }}</p>
          <p v-else-if="!revisions.length" class="p-3 text-xs text-muted-foreground">{{ t("specification.historyEmpty") }}</p>
          <button
            v-for="revision in revisions"
            :key="revision.version"
            type="button"
            class="flex w-full flex-col gap-1 rounded px-3 py-2 text-left text-xs hover:bg-accent"
            :class="selectedVersion === revision.version ? 'bg-accent text-foreground' : 'text-muted-foreground'"
            @click="selectedVersion = revision.version"
          >
            <span class="flex items-center justify-between gap-2 font-medium"
              ><span>{{ t("specification.revision", { version: revision.version }) }}</span
              ><span v-if="revision.version === specification.version" class="text-[10px]">{{ t("specification.current") }}</span></span
            >
            <span>{{ t("specification.revisionBy", { actor: revision.createdBy, date: formatDate(revision.createdAt, locale) }) }}</span>
          </button>
        </nav>
        <div class="min-h-0 overflow-y-auto overscroll-contain rounded-md border border-border p-4">
          <div v-if="selected" class="space-y-3">
            <div class="flex flex-wrap items-center justify-between gap-2 border-b border-border pb-3">
              <div>
                <strong>{{ t("specification.revision", { version: selected.version }) }}</strong>
                <p class="text-xs text-muted-foreground">{{ t("specification.revisionBy", { actor: selected.createdBy, date: formatDate(selected.createdAt, locale) }) }}</p>
              </div>
              <Button v-if="selected.version !== specification.version" size="sm" variant="outline" :disabled="hasDraft" :title="hasDraft ? t('specification.saveBeforeRestore') : undefined" @click="restoreVersion = selected.version"
                ><RotateCcw class="h-3.5 w-3.5" />{{ t("specification.restoreVersion") }}</Button
              >
            </div>
            <p v-if="hasDraft" class="rounded-md border border-amber-300 bg-amber-50 p-2 text-xs text-amber-950 dark:border-amber-700 dark:bg-amber-950/30 dark:text-amber-100">{{ t("specification.saveBeforeRestore") }}</p>
            <div class="grid gap-3 lg:grid-cols-2">
              <div>
                <div class="mb-2 flex items-center gap-1 text-xs font-medium text-muted-foreground"><Eye class="h-3.5 w-3.5" />{{ t("specification.revision", { version: selected.version }) }}</div>
                <MarkdownPreview :content="selected.content" />
              </div>
              <div v-if="selected.version !== specification.version">
                <div class="mb-2 text-xs font-medium text-muted-foreground">{{ t("specification.current") }}</div>
                <MarkdownPreview :content="specification.content" />
              </div>
            </div>
          </div>
          <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t("specification.historyEmpty") }}</p>
        </div>
      </div>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <DialogFooter
        ><Button variant="outline" @click="open = false">{{ t("common.close") }}</Button></DialogFooter
      >
    </DialogContent>
  </Dialog>
  <Dialog :open="restoreVersion !== null" @update:open="(value) => !value && (restoreVersion = null)">
    <DialogContent>
      <DialogHeader
        ><DialogTitle>{{ t("specification.restoreConfirmTitle", { version: restoreVersion ?? "" }) }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("specification.restoreConfirmDescription") }}</p>
      <DialogFooter
        ><Button variant="outline" :disabled="busy" @click="restoreVersion = null">{{ t("common.cancel") }}</Button
        ><Button :disabled="busy" @click="restore"><RotateCcw class="h-3.5 w-3.5" />{{ busy ? t("common.loading") : t("common.restore") }}</Button></DialogFooter
      >
    </DialogContent>
  </Dialog>
</template>
