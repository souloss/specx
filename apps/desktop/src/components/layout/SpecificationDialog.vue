<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowLeft, ArrowRight, Check, FileText, Grid2X2, List, Search } from "@lucide/vue";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useWorkspaceStore } from "@/stores/workspace";
import { specxApi } from "@/lib/backend/client";
import { createId } from "@/lib/core";
import { SPECIFICATION_TYPES } from "@/lib/specifications/registry";
import type { SpecificationKind } from "@/types/domain";
import { translate, type MessageKey } from "@/i18n";

const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const step = ref<1 | 2 | 3>(1);
const query = ref("");
const selectedKind = ref<SpecificationKind>("markdown");
const title = ref("");
const content = ref("");
const error = ref("");
const busy = ref(false);
const serviceId = ref("");
const stepLabels = ["specification.step1", "specification.step2", "specification.step3"] as const;
const viewMode = ref<"grid" | "list">("grid");
const categoryFilter = ref<"all" | "document">("all");
const selectedType = computed(() => SPECIFICATION_TYPES.find((item) => item.kind === selectedKind.value) ?? SPECIFICATION_TYPES[0]!);
const categories = computed(() => ["all", ...new Set(SPECIFICATION_TYPES.map((item) => item.category))] as const);
const visibleTypes = computed(() => {
  const normalized = query.value.trim().toLocaleLowerCase();
  return SPECIFICATION_TYPES.filter((item) => item.enabled && (categoryFilter.value === "all" || item.category === categoryFilter.value) && (!normalized || [t(item.titleKey), t(item.descriptionKey), t(item.keywordsKey)].join(" ").toLocaleLowerCase().includes(normalized)));
});
const canContinue = computed(() => Boolean(selectedKind.value) && (step.value !== 2 || Boolean(title.value.trim() && content.value.trim())));

watch(open, (value) => {
  if (!value) return;
  step.value = 1;
  query.value = "";
  viewMode.value = "grid";
  categoryFilter.value = "all";
  selectedKind.value = "markdown";
  title.value = "";
  content.value = t("specification.defaultContent");
  error.value = "";
  serviceId.value = workspace.activeServiceId ?? "";
});
function selectType(kind: SpecificationKind) {
  selectedKind.value = kind;
  if (kind === "markdown" && !content.value) content.value = t("specification.defaultContent");
}
function next() {
  if (!canContinue.value || step.value === 3) return;
  step.value = (step.value + 1) as 2 | 3;
}
function previous() {
  if (step.value > 1) step.value = (step.value - 1) as 1 | 2;
}
async function save() {
  if (!workspace.catalog || !serviceId.value || busy.value || selectedKind.value !== "markdown") return;
  busy.value = true;
  error.value = "";
  try {
    const id = createId("spec");
    await specxApi.createSpecification(workspace.catalog.workspace.id, { id, serviceId: serviceId.value, title: title.value.trim(), content: content.value });
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
    <DialogContent class="sm:max-w-2xl" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()">
      <DialogHeader>
        <DialogTitle>{{ t("specification.createTitle") }}</DialogTitle>
        <div class="flex flex-wrap items-center gap-2 pt-2 text-xs text-muted-foreground" :aria-label="t('specification.steps')">
          <span v-for="index in 3" :key="index" class="flex items-center gap-1" :class="step >= index ? 'text-foreground' : ''"
            ><span class="flex h-5 w-5 items-center justify-center rounded-full border" :class="step >= index ? 'border-primary bg-primary text-primary-foreground' : 'border-border'">{{ index }}</span
            >{{ t(stepLabels[index - 1]!) }}</span
          >
        </div>
      </DialogHeader>
      <form class="flex min-h-0 flex-col gap-4" @submit.prevent="step === 3 ? save() : next()">
        <section v-if="step === 1" class="flex min-h-0 flex-col gap-3">
          <div class="flex gap-2">
            <div class="relative min-w-0 flex-1">
              <Search class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" /><input v-model="query" class="settings-input pl-9" :aria-label="t('specification.searchTypes')" :placeholder="t('specification.searchTypes')" />
            </div>
            <button type="button" class="rounded border p-2" :class="viewMode === 'grid' ? 'bg-accent text-foreground' : 'text-muted-foreground'" :aria-label="t('specification.gridView')" :aria-pressed="viewMode === 'grid'" @click="viewMode = 'grid'"><Grid2X2 class="h-4 w-4" /></button>
            <button type="button" class="rounded border p-2" :class="viewMode === 'list' ? 'bg-accent text-foreground' : 'text-muted-foreground'" :aria-label="t('specification.listView')" :aria-pressed="viewMode === 'list'" @click="viewMode = 'list'"><List class="h-4 w-4" /></button>
          </div>
          <div class="flex gap-1 overflow-x-auto border-b border-border pb-2">
            <button v-for="category in categories" :key="category" type="button" class="shrink-0 rounded px-2 py-1 text-xs" :class="categoryFilter === category ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent'" @click="categoryFilter = category">
              <span v-if="category === 'all'">{{ t("specification.categories.all") }}</span
              ><span v-else>{{ t(`specification.categories.${category}` as MessageKey) }}</span>
            </button>
          </div>
          <div class="max-h-64 gap-2 overflow-y-auto" :class="viewMode === 'grid' ? 'grid sm:grid-cols-2' : 'flex flex-col'">
            <button v-for="item in visibleTypes" :key="item.kind" type="button" class="flex items-start gap-3 rounded-md border p-3 text-left transition-colors hover:bg-accent" :class="selectedKind === item.kind ? 'border-primary bg-accent' : 'border-border'" @click="selectType(item.kind)">
              <FileText class="mt-0.5 h-5 w-5 shrink-0 text-primary" />
              <span class="min-w-0"
                ><strong class="block text-sm font-medium">{{ t(item.titleKey) }}</strong
                ><span class="mt-1 block text-xs text-muted-foreground">{{ t(item.descriptionKey) }}</span></span
              >
            </button>
          </div>
          <p v-if="!visibleTypes.length" class="py-8 text-center text-sm text-muted-foreground">{{ t("specification.noTypes") }}</p>
        </section>
        <section v-else-if="step === 2" class="flex min-h-0 flex-col gap-4">
          <div class="rounded-md border border-border bg-muted/20 p-3">
            <div class="text-sm font-medium">{{ t(selectedType.titleKey) }}</div>
            <div class="text-xs text-muted-foreground">{{ t(selectedType.descriptionKey) }}</div>
          </div>
          <label class="settings-field">{{ t("specification.title") }}<input v-model="title" class="settings-input" required autofocus /></label>
          <label class="settings-field">{{ t("specification.content") }}<textarea v-model="content" class="settings-input min-h-56 resize-y font-mono" rows="12" required /></label>
        </section>
        <section v-else class="flex flex-col gap-3">
          <div class="rounded-md border border-border p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium"><Check class="h-4 w-4 text-primary" />{{ t("specification.ready") }}</div>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
              <dt class="text-muted-foreground">{{ t("specification.type") }}</dt>
              <dd>{{ t(selectedType.titleKey) }}</dd>
              <dt class="text-muted-foreground">{{ t("specification.title") }}</dt>
              <dd class="truncate">{{ title }}</dd>
            </dl>
          </div>
          <p class="text-xs text-muted-foreground">{{ t("specification.confirmDescription") }}</p>
        </section>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <DialogFooter>
          <Button type="button" variant="outline" :disabled="busy" @click="step === 1 ? (open = false) : previous()"><ArrowLeft v-if="step > 1" class="h-3.5 w-3.5" />{{ step === 1 ? t("common.cancel") : t("common.back") }}</Button>
          <Button v-if="step < 3" type="submit" :disabled="!canContinue"><ArrowRight class="h-3.5 w-3.5" />{{ t("common.next") }}</Button>
          <Button v-else type="submit" :disabled="busy || !title.trim() || !content.trim()">{{ busy ? t("common.creating") : t("specification.create") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
