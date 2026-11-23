<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Check, Layers, Search } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { translate } from "@/i18n";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { getOpenApiModels } from "@/lib/specifications/openapi/contract/openapiModels";
import OpenApiSchemaPreview from "./OpenApiSchemaPreview.vue";

const props = defineProps<{ document: OpenApiObject | null; currentReference?: string; label: string }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ select: [reference: string] }>();
const t = translate;
const query = ref("");
const selectedReference = ref("");
const models = computed(() => getOpenApiModels(props.document));
const filteredModels = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase();
  return needle ? models.value.filter((model) => `${model.name} ${JSON.stringify(model.schema)}`.toLocaleLowerCase().includes(needle)) : models.value;
});
const selected = computed(() => models.value.find((model) => model.reference === selectedReference.value));
watch(
  open,
  (value) => {
    if (!value) return;
    query.value = "";
    selectedReference.value = props.currentReference ?? "";
  },
  { immediate: true },
);
function select() {
  if (!selected.value) return;
  emit("select", selected.value.reference);
  open.value = false;
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogScrollContent class="schema-model-picker grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 sm:max-w-3xl">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle class="flex items-center gap-2 text-base"><Layers class="size-4" aria-hidden="true" />{{ t("specification.openapiModels.pickerTitle") }}</DialogTitle>
        <DialogDescription>{{ t("specification.openapiModels.pickerHint", { label: props.label }) }}</DialogDescription>
      </DialogHeader>
      <div class="schema-model-picker-body">
        <div class="schema-model-picker-catalog">
          <label class="relative block">
            <Search class="pointer-events-none absolute left-3 top-2.5 size-4 text-muted-foreground" aria-hidden="true" />
            <Input v-model="query" name="openapi-model-search" class="pl-9" :aria-label="t('specification.openapiModels.search')" :placeholder="t('specification.openapiModels.search')" autocomplete="off" :spellcheck="false" />
          </label>
          <fieldset class="schema-model-picker-list">
            <legend class="sr-only">{{ t("specification.openapiModels.directory") }}</legend>
            <label v-for="model in filteredModels" :key="model.reference" class="schema-model-picker-option" :class="{ selected: selectedReference === model.reference }">
              <input v-model="selectedReference" type="radio" name="openapi-model-choice" :value="model.reference" :aria-label="model.name" />
              <span class="min-w-0 flex-1"
                ><strong>{{ model.name }}</strong
                ><span v-if="model.description" class="line-clamp-2 text-xs text-muted-foreground">{{ model.description }}</span
                ><code>{{ model.reference }}</code></span
              >
              <Check v-if="selectedReference === model.reference" class="size-4 shrink-0 text-primary" aria-hidden="true" />
            </label>
            <p v-if="!filteredModels.length" class="p-3 text-sm text-muted-foreground" role="status">{{ t(models.length ? "specification.openapiModels.noResults" : "specification.openapiModels.pickerEmpty") }}</p>
          </fieldset>
        </div>
        <section class="schema-model-picker-preview" :aria-label="t('specification.openapiModels.pickerPreview')">
          <template v-if="selected"
            ><h3>{{ selected.name }}</h3>
            <OpenApiSchemaPreview :key="selected.reference" :schema="{ $ref: selected.reference }" :document="props.document" mode="full" />
            <slot name="constraints" :model="selected" />
          </template>
          <p v-else class="text-sm text-muted-foreground">{{ t("specification.openapiModels.pickerPreviewHint") }}</p>
        </section>
      </div>
      <DialogFooter class="m-0 rounded-b-lg border-t border-border px-5 py-4">
        <span class="mr-auto self-center text-xs text-muted-foreground" aria-live="polite">{{ selected?.name }}</span>
        <Button variant="outline" @click="open = false">{{ t("common.cancel") }}</Button>
        <Button :disabled="!selected" @click="select"><Layers class="size-3.5" aria-hidden="true" />{{ t("specification.openapiModels.pickerConfirm") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
