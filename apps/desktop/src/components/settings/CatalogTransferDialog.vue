<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { useWorkspaceStore } from "@/stores/workspace";
import { parseCatalogBundle, importCatalog, type CatalogBundle } from "@/lib/transfer/catalogTransfer";
import { translate } from "@/i18n";
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const bundle = ref<CatalogBundle | null>(null);
const error = ref("");
const busy = ref(false);
const completed = ref(0);
const total = ref(0);
const fileName = ref("");
const finished = ref(false);
const partial = computed(() => !!error.value && completed.value > 0);
watch(open, (value) => {
  if (value && !busy.value) {
    bundle.value = null;
    error.value = "";
    completed.value = 0;
    total.value = 0;
    fileName.value = "";
    finished.value = false;
  }
});
async function selectFile(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  bundle.value = null;
  error.value = "";
  if (!file) return;
  try {
    if (file.size > 10 * 1024 * 1024) throw new Error(t("transfer.fileTooLarge"));
    const parsed = parseCatalogBundle(await file.text());
    const conflict = parsed.services.find((service) => workspace.catalog?.services.some((existing) => existing.slug === service.slug));
    if (conflict) throw new Error(t("transfer.slugConflict", { slug: conflict.slug }));
    bundle.value = parsed;
    fileName.value = file.name;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("transfer.readFailed");
  }
}
async function runImport() {
  if (!bundle.value || !workspace.catalog || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await importCatalog(bundle.value, workspace.catalog, (count, size) => {
      completed.value = count;
      total.value = size;
    });
    finished.value = true;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("transfer.importFailed");
  } finally {
    try {
      await workspace.refresh();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : t("sidebar.refreshFailed");
    }
    busy.value = false;
  }
}
</script>

<template>
  <Dialog
    :open="open"
    @update:open="
      (value) => {
        if (!busy) open = value;
      }
    "
  >
    <DialogContent class="sm:max-w-lg" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()">
      <DialogHeader
        ><DialogTitle>{{ t("transfer.title") }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("transfer.description") }}</p>
      <input v-if="!busy && !completed && !finished" type="file" accept=".json,application/json" :aria-label="t('transfer.file')" class="text-sm" @change="selectFile" />
      <div v-if="bundle" class="rounded-md border border-border bg-muted/20 p-3 text-sm">
        <p class="mb-2 truncate font-medium">{{ fileName }}</p>
        <p>{{ t("transfer.groupCount", { count: bundle.groups.length }) }} · {{ t("transfer.tagCount", { count: bundle.tags.length }) }} · {{ t("transfer.serviceCount", { count: bundle.services.length }) }} · {{ t("transfer.specificationCount", { count: bundle.specifications.length }) }}</p>
      </div>
      <p v-if="busy || completed" role="status" class="text-sm">{{ t("transfer.progress", { status: finished ? t("transfer.completed") : t("transfer.saved"), completed, total }) }}</p>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <p v-if="partial" class="text-xs text-muted-foreground">{{ t("transfer.partial") }}</p>
      <DialogFooter
        ><Button variant="outline" :disabled="busy" @click="open = false">{{ finished || completed ? t("common.close") : t("common.cancel") }}</Button
        ><Button v-if="!finished && !completed" :disabled="!bundle || busy || !!error" @click="runImport">{{ busy ? t("transfer.importing") : t("transfer.confirm") }}</Button></DialogFooter
      >
    </DialogContent>
  </Dialog>
</template>
