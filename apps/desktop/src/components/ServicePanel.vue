<script setup lang="ts">
import { computed } from "vue";
import { FileText, Plus } from "@lucide/vue";
import MarkdownPreview from "./MarkdownPreview.vue";
import { formatDate, groupPath } from "../lib/core";
import { i18n, translate } from "@/i18n";
import type { Catalog, PanelTab } from "../types/domain";

const props = defineProps<{ catalog: Catalog; serviceId: string; tab: PanelTab }>();
const emit = defineEmits<{ openSpecification: [id: string]; newSpecification: [] }>();
const service = computed(() => props.catalog.services.find((item) => item.id === props.serviceId));
const activeSpecification = computed(() => (props.tab.specificationId ? props.catalog.specifications.find((item) => item.id === props.tab.specificationId) : undefined));
const specifications = computed(() => props.catalog.specifications.filter((item) => item.serviceId === props.serviceId));
const tags = computed(() => service.value?.tagIds.map((id) => props.catalog.tags.find((tag) => tag.id === id)).filter(Boolean) ?? []);
const t = translate;
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
</script>

<template>
  <div v-if="!service" class="panel-empty">
    <h2>{{ t("service.unavailable") }}</h2>
    <p>{{ t("service.unavailableDescription") }}</p>
  </div>
  <div v-else-if="activeSpecification" class="spec-panel">
    <header class="spec-toolbar">
      <div>
        <span class="eyebrow">{{ t("service.specificationHeading") }}</span>
        <h2>{{ activeSpecification.title }}</h2>
        <span class="muted">{{ service.name }} · {{ t("service.updatedAt", { date: formatDate(activeSpecification.updatedAt, locale) }) }}</span>
      </div>
    </header>
    <div class="spec-layout">
      <MarkdownPreview :content="activeSpecification.content" />
      <aside class="spec-meta">
        <span class="eyebrow">{{ t("service.serviceLabel") }}</span
        ><strong>{{ service.name }}</strong
        ><span>{{ service.slug }}</span>
      </aside>
    </div>
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
