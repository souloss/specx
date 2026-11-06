<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Edit3, FileText, History, Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import MarkdownPreview from "./MarkdownPreview.vue";
import SpecificationEditor from "./SpecificationEditor.vue";
import SpecificationHistory from "./SpecificationHistory.vue";
import { formatDate, groupPath } from "../lib/core";
import { i18n, translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Catalog, PanelTab } from "../types/domain";

const props = defineProps<{ catalog: Catalog; serviceId: string; tab: PanelTab }>();
const emit = defineEmits<{ openSpecification: [id: string]; newSpecification: [] }>();
const workspace = useWorkspaceStore();
const service = computed(() => props.catalog.services.find((item) => item.id === props.serviceId));
const activeSpecification = computed(() => (props.tab.specificationId ? props.catalog.specifications.find((item) => item.id === props.tab.specificationId) : undefined));
const specifications = computed(() => props.catalog.specifications.filter((item) => item.serviceId === props.serviceId));
const tags = computed(() => service.value?.tagIds.map((id) => props.catalog.tags.find((tag) => tag.id === id)).filter(Boolean) ?? []);
const t = translate;
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
const editing = ref(false);
const historyOpen = ref(false);
const deleteOpen = ref(false);
const deleting = ref(false);
const error = ref("");

watch(
  () => activeSpecification.value?.id,
  () => {
    editing.value = false;
    historyOpen.value = false;
    deleteOpen.value = false;
    error.value = "";
  },
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
</script>

<template>
  <div v-if="!service" class="panel-empty">
    <h2>{{ t("service.unavailable") }}</h2>
    <p>{{ t("service.unavailableDescription") }}</p>
  </div>
  <div v-else-if="activeSpecification" class="spec-panel flex min-h-0 flex-col">
    <header class="spec-toolbar">
      <div>
        <span class="eyebrow">{{ t("service.specificationHeading") }}</span>
        <h2>{{ activeSpecification.title }}</h2>
        <span class="muted">{{ service.name }} · {{ t("service.updatedAt", { date: formatDate(activeSpecification.updatedAt, locale) }) }} · {{ t("specification.currentVersion", { version: activeSpecification.version }) }}</span>
      </div>
      <div v-if="!editing" class="flex flex-wrap items-center gap-2">
        <Button size="sm" @click="editing = true"><Edit3 class="h-3.5 w-3.5" />{{ t("service.editSpecification") }}</Button>
        <Button size="sm" variant="outline" @click="historyOpen = true"><History class="h-3.5 w-3.5" />{{ t("service.specificationHistory") }}</Button>
        <Button size="sm" variant="destructive" @click="deleteOpen = true"><Trash2 class="h-3.5 w-3.5" />{{ t("service.deleteSpecification") }}</Button>
      </div>
    </header>
    <SpecificationEditor v-if="editing" :specification="activeSpecification" @close="editing = false" />
    <div v-else class="spec-layout">
      <MarkdownPreview :content="activeSpecification.content" />
      <aside class="spec-meta">
        <span class="eyebrow">{{ t("service.serviceLabel") }}</span
        ><strong>{{ service.name }}</strong
        ><span>{{ service.slug }}</span>
        <span class="mt-3 text-xs">{{ t("specification.currentVersion", { version: activeSpecification.version }) }}</span>
      </aside>
    </div>
    <SpecificationHistory v-model:open="historyOpen" :specification="activeSpecification" />
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
  <div v-else class="overview-panel">
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
      <button class="button primary" @click="emit('newSpecification')"><Plus :size="16" />{{ t("service.createSpecification") }}</button>
    </header>
    <section class="content-section">
      <div class="section-heading">
        <div>
          <span class="eyebrow">{{ t("service.specifications") }}</span>
          <h3>
            {{ t("service.specifications") }} <small>{{ specifications.length }}</small>
          </h3>
        </div>
        <button class="text-button" @click="emit('newSpecification')"><Plus :size="14" />{{ t("service.create") }}</button>
      </div>
      <div v-if="!specifications.length" class="section-empty">
        <FileText :size="22" /><strong>{{ t("service.noSpecifications") }}</strong
        ><span>{{ t("service.addFirstSpecification") }}</span>
      </div>
      <div v-else class="spec-list">
        <button v-for="specification in specifications" :key="specification.id" class="spec-row" @click="emit('openSpecification', specification.id)">
          <FileText :size="17" /><span
            ><strong>{{ specification.title }}</strong
            ><small>Markdown · {{ t("service.updatedAt", { date: formatDate(specification.updatedAt, locale) }) }}</small></span
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
        <div>
          <span>{{ t("service.format") }}</span
          ><strong>Markdown</strong>
        </div>
      </div>
      <div class="tag-list">
        <span v-for="tag in tags" :key="tag!.id" class="tag-chip"><i :style="{ backgroundColor: tag!.color }" />{{ tag!.name }}</span
        ><span v-if="!tags.length" class="muted">{{ t("service.noTags") }}</span>
      </div>
    </section>
  </div>
</template>
