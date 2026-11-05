<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowLeft, ArrowRight, Check, FileText, Grid2X2, List, Search } from "@lucide/vue";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useWorkspaceStore } from "@/stores/workspace";
import { specxApi } from "@/lib/backend/client";
import { createId } from "@/lib/core";
import { SPECIFICATION_TYPES, type SpecificationFieldDefinition, type SpecificationTypeDefinition } from "@/lib/specifications/registry";
import type { SpecificationKind } from "@/types/domain";
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
const canContinue = computed(() => {
  if (currentStep.value.kind === "type-picker") return Boolean(selectedKind.value);
  return currentStep.value.fields?.every((field) => !field.required || fieldValue(field).trim()) !== false;
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
  error.value = "";
  serviceId.value = workspace.activeServiceId ?? "";
});
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
    await specxApi.createSpecification(workspace.catalog.workspace.id, selectedType.value.createRequest({ id, serviceId: serviceId.value, values: values.value }));
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
          <span v-for="(item, index) in selectedType.steps" :key="item.id" class="flex items-center gap-1" :class="step >= index + 1 ? 'text-foreground' : ''"
            ><span class="flex h-5 w-5 items-center justify-center rounded-full border" :class="step >= index + 1 ? 'border-primary bg-primary text-primary-foreground' : 'border-border'">{{ index + 1 }}</span
            >{{ t(item.labelKey) }}</span
          >
        </div>
      </DialogHeader>
      <form class="flex min-h-0 flex-col gap-4" @submit.prevent="step === selectedType.steps.length ? save() : next()">
        <section v-if="currentStep.kind === 'type-picker'" class="flex min-h-0 flex-col gap-3">
          <div class="flex gap-2">
            <div class="relative min-w-0 flex-1">
              <Search class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" /><input v-model="query" class="settings-input pl-9" :aria-label="t('specification.searchTypes')" :placeholder="t('specification.searchTypes')" />
            </div>
            <button type="button" class="rounded border p-2" :class="viewMode === 'grid' ? 'bg-accent text-foreground' : 'text-muted-foreground'" :aria-label="t('specification.gridView')" :aria-pressed="viewMode === 'grid'" @click="viewMode = 'grid'"><Grid2X2 class="h-4 w-4" /></button>
            <button type="button" class="rounded border p-2" :class="viewMode === 'list' ? 'bg-accent text-foreground' : 'text-muted-foreground'" :aria-label="t('specification.listView')" :aria-pressed="viewMode === 'list'" @click="viewMode = 'list'"><List class="h-4 w-4" /></button>
          </div>
          <div class="flex gap-1 overflow-x-auto border-b border-border pb-2">
            <button v-for="category in categories" :key="category.id" type="button" class="shrink-0 rounded px-2 py-1 text-xs" :class="categoryFilter === category.id ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent'" @click="categoryFilter = category.id">
              {{ t(category.labelKey) }}
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
        <section v-else-if="currentStep.kind === 'fields'" class="flex min-h-0 flex-col gap-4">
          <div class="rounded-md border border-border bg-muted/20 p-3">
            <div class="text-sm font-medium">{{ t(selectedType.titleKey) }}</div>
            <div class="text-xs text-muted-foreground">{{ t(selectedType.descriptionKey) }}</div>
          </div>
          <label v-for="field in currentStep.fields ?? []" :key="field.id" class="settings-field">
            {{ t(field.labelKey) }}
            <input v-if="field.control === 'text'" :value="fieldValue(field)" class="settings-input" :required="field.required" autofocus @input="updateField(field, $event)" />
            <textarea v-else :value="fieldValue(field)" class="settings-input min-h-56 resize-y font-mono" :rows="field.rows ?? 8" :required="field.required" @input="updateField(field, $event)" />
          </label>
        </section>
        <section v-else class="flex flex-col gap-3">
          <div class="rounded-md border border-border p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium"><Check class="h-4 w-4 text-primary" />{{ t("specification.ready") }}</div>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
              <dt class="text-muted-foreground">{{ t("specification.type") }}</dt>
              <dd>{{ t(selectedType.titleKey) }}</dd>
              <template v-for="summaryField in currentStep.summaryFields ?? []" :key="summaryField.id">
                <dt class="text-muted-foreground">{{ t(summaryField.labelKey) }}</dt>
                <dd class="truncate">{{ fieldValue(summaryField) }}</dd>
              </template>
            </dl>
          </div>
          <p class="text-xs text-muted-foreground">{{ t("specification.confirmDescription") }}</p>
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
