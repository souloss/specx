<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { ArrowLeft, ChevronDown, Search } from "@lucide/vue";
import OpenApiDocumentMetadata from "./OpenApiDocumentMetadata.vue";
import MarkdownPreview from "../../markdown/MarkdownPreview.vue";
import OpenApiPublishedOperation from "../publishing/OpenApiPublishedOperation.vue";
import OpenApiPublishedModel from "../publishing/OpenApiPublishedModel.vue";
import { getOpenApiOperations, getOpenApiServers, isRecord, openApiMethodClass, parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import { getOpenApiModels } from "@/lib/specifications/openapi/contract/openapiModels";
import { buildOpenApiSearchIndex, matchesOpenApiSearch } from "@/lib/specifications/openapi/documentation/openapiDocumentSearch";
import { translate as t } from "@/i18n";

const props = withDefaults(defineProps<{ allowExport?: boolean; content: string; title: string; version: number; linked?: boolean; allowTryIt?: boolean }>(), { linked: false, allowTryIt: false, allowExport: true });
const selected = defineModel<string>("operation", { default: "" });
const filter = ref("");
const searchInput = ref<HTMLInputElement>();
async function focusSearch() {
  catalogueOpen.value = true;
  await nextTick();
  searchInput.value?.focus();
}
defineExpose({ focusSearch });
const catalogueOpen = ref(!globalThis.matchMedia?.("(max-width: 767px)").matches);
const catalogue = ref<"operations" | "models">("operations");
const document = computed(() => parseOpenApiContent(props.content));
const description = computed(() => (isRecord(document.value?.info) && typeof document.value.info.description === "string" ? document.value.info.description : ""));
const infoVersion = computed(() => (isRecord(document.value?.info) && typeof document.value.info.version === "string" ? document.value.info.version : ""));
const operations = computed(() => getOpenApiOperations(props.content));
const models = computed(() => getOpenApiModels(document.value));
const searchIndex = computed(() => buildOpenApiSearchIndex(document.value, operations.value, models.value));
const matchingOperations = computed(() => operations.value.filter((entry) => matchesOpenApiSearch(searchIndex.value.operations.get(entry.identityKey) ?? "", filter.value)));
const activeTag = computed(() => {
  if (!selected.value.startsWith("tag:")) return undefined;
  const tag = selected.value.slice(4);
  return operations.value.some((entry) => entry.tags.includes(tag)) || (Array.isArray(document.value?.tags) && document.value.tags.some((entry) => isRecord(entry) && entry.name === tag)) ? tag : undefined;
});
const tagMetadata = computed(() => (Array.isArray(document.value?.tags) ? document.value.tags.filter(isRecord).find((tag) => tag.name === activeTag.value) : undefined));
const active = computed(() => operations.value.find((entry) => entry.identityKey === selected.value));
const activeModel = computed(() => models.value.find((entry) => `model:${entry.reference}` === selected.value));
const servers = computed(() => getOpenApiServers(document.value, undefined));
const pages = computed(() => [...operations.value.map((entry) => ({ key: entry.identityKey, label: `${entry.method.toUpperCase()} ${entry.path}` })), ...models.value.map((model) => ({ key: `model:${model.reference}`, label: model.name }))]);
const adjacent = computed(() => {
  const index = pages.value.findIndex((page) => page.key === selected.value);
  return index < 0
    ? []
    : [
        { direction: "previousPage" as const, page: pages.value[index - 1] },
        { direction: "nextPage" as const, page: pages.value[index + 1] },
      ].filter((item) => item.page);
});
watch(
  selected,
  (value) => {
    if (!value) return;
    catalogue.value = value.startsWith("model:") ? "models" : "operations";
  },
  { immediate: true },
);
const heading = ref<HTMLElement | null>(null);
const groups = computed(() => {
  const grouped = new Map<string, typeof operations.value>();
  for (const entry of matchingOperations.value) {
    for (const tag of entry.tags) {
      let group = grouped.get(tag);
      if (!group) grouped.set(tag, (group = []));
      group.push(entry);
    }
  }
  return [...grouped].sort(([a], [b]) => a.localeCompare(b));
});
const filteredModels = computed(() => models.value.filter((model) => matchesOpenApiSearch(searchIndex.value.models.get(model.reference) ?? "", filter.value)));
const overviewGroups = computed<Array<[string, typeof operations.value]>>(() => (activeTag.value === undefined ? groups.value : [[activeTag.value, operations.value.filter((entry) => entry.tags.includes(activeTag.value!))]]));

async function choose(key: string) {
  selected.value = key;
  if (globalThis.matchMedia?.("(max-width: 767px)").matches) catalogueOpen.value = false;
  await nextTick();
  const target = heading.value?.querySelector<HTMLElement>("h2") ?? heading.value;
  target?.focus({ preventScroll: true });
  heading.value?.scrollIntoView({ block: "start" });
}

function chooseCatalogue(value: "operations" | "models") {
  catalogue.value = value;
  // Keep the catalogue choice authoritative. The selection watcher only
  // follows an existing deep link and should not switch the user back while
  // they are browsing the other catalogue.
  if (value === "operations" && selected.value.startsWith("model:")) selected.value = "";
  if (value === "models" && !selected.value.startsWith("model:")) selected.value = "";
}

function chooseModel(reference: string) {
  choose(`model:${reference}`);
}
</script>

<template>
  <div class="min-w-0 md:grid md:grid-cols-[minmax(13rem,17rem)_minmax(0,1fr)]">
    <aside class="min-w-0 border-b border-border bg-surface px-3 py-3 md:border-r md:border-b-0">
      <button type="button" class="flex w-full items-center justify-between rounded px-2 py-1.5 text-sm font-semibold hover:bg-accent md:hidden" :aria-expanded="catalogueOpen" @click="catalogueOpen = !catalogueOpen">
        {{ t("openapiPublication.catalogue") }}<ChevronDown class="size-4 transition-transform" :class="{ 'rotate-180': catalogueOpen }" aria-hidden="true" />
      </button>
      <nav :aria-label="t('openapiPublication.catalogue')" :class="[catalogueOpen ? '' : 'hidden md:block', 'md:sticky md:top-4']">
        <div class="mt-2 grid grid-cols-2 gap-1 rounded-md border border-border bg-muted/30 p-1 md:mt-3">
          <button
            type="button"
            class="rounded px-2 py-1.5 text-xs font-medium transition-colors"
            :class="catalogue === 'operations' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'"
            :aria-pressed="catalogue === 'operations'"
            @click="chooseCatalogue('operations')"
          >
            {{ t("openapiPublication.operations") }}<span v-if="filter" class="ml-1 text-muted-foreground">{{ matchingOperations.length }}</span>
          </button>
          <button type="button" class="rounded px-2 py-1.5 text-xs font-medium transition-colors" :class="catalogue === 'models' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'" :aria-pressed="catalogue === 'models'" @click="chooseCatalogue('models')">
            {{ t("openapiPublication.models") }}<span v-if="filter" class="ml-1 text-muted-foreground">{{ filteredModels.length }}</span>
          </button>
        </div>
        <label class="relative mt-2 block">
          <Search class="pointer-events-none absolute top-2.5 left-2.5 size-4 text-muted-foreground" aria-hidden="true" />
          <input ref="searchInput" v-model="filter" type="search" name="openapi-publication-search" autocomplete="off" :spellcheck="false" class="field-control w-full pl-8!" :placeholder="t('openapiPublication.search')" :aria-label="t('openapiPublication.search')" />
        </label>
        <p v-if="filter" role="status" class="px-2 pt-2 text-[11px] leading-5 text-muted-foreground">{{ t("openapiPublication.searchResults", { operations: matchingOperations.length, models: filteredModels.length }) }}</p>
        <div v-if="catalogue === 'operations'" class="mt-2 max-h-[min(68vh,48rem)] space-y-2 overflow-y-auto overscroll-contain py-1">
          <p v-if="!groups.length" class="px-2 py-2 text-xs text-muted-foreground">{{ t("openapiPublication.noOperations") }}</p>
          <details v-for="[tag, entries] in groups" :key="tag" open class="min-w-0">
            <summary class="cursor-pointer rounded px-2 py-1 text-xs font-semibold break-words text-muted-foreground hover:bg-accent">
              <component
                :is="props.linked ? 'a' : 'button'"
                :href="props.linked ? '#' + encodeURIComponent(`tag:${tag}`) : undefined"
                :type="props.linked ? undefined : 'button'"
                class="rounded text-left hover:text-foreground"
                :aria-current="activeTag === tag ? 'page' : undefined"
                @click.exact.stop.prevent="choose(`tag:${tag}`)"
                >{{ tag }}</component
              >
            </summary>
            <ul class="mt-1">
              <li v-for="entry in entries" :key="entry.identityKey" class="openapi-catalogue-row">
                <component
                  :is="props.linked ? 'a' : 'button'"
                  :href="props.linked ? '#' + encodeURIComponent(entry.identityKey) : undefined"
                  :type="props.linked ? undefined : 'button'"
                  :aria-current="active?.identityKey === entry.identityKey ? 'page' : undefined"
                  class="flex w-full min-w-0 items-baseline gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
                  :class="{ 'bg-accent font-semibold': active?.identityKey === entry.identityKey }"
                  @click.exact.prevent="choose(entry.identityKey)"
                >
                  <span class="w-12 shrink-0 rounded px-1 py-0.5 text-center font-mono text-[10px] font-semibold" :class="openApiMethodClass(entry.method)" translate="no">{{ entry.method.toUpperCase() }}</span
                  ><span class="min-w-0"
                    ><span class="block break-words">{{ entry.operation.summary || entry.operation.operationId || entry.path }}</span
                    ><span v-if="entry.operation.summary || entry.operation.operationId" class="mt-0.5 block break-all font-mono text-[10px] font-normal text-muted-foreground">{{ entry.path }}</span></span
                  >
                </component>
              </li>
            </ul>
          </details>
        </div>
        <div v-else class="mt-2 max-h-[min(68vh,48rem)] space-y-1 overflow-y-auto overscroll-contain py-1">
          <p v-if="!filteredModels.length" class="px-2 py-2 text-xs text-muted-foreground">{{ t("openapiPublication.noModels") }}</p>
          <component
            v-for="model in filteredModels"
            :key="model.reference"
            :is="props.linked ? 'a' : 'button'"
            :href="props.linked ? '#' + encodeURIComponent(`model:${model.reference}`) : undefined"
            :type="props.linked ? undefined : 'button'"
            class="openapi-catalogue-row flex w-full min-w-0 items-start gap-2 rounded px-2 py-2 text-left text-xs hover:bg-accent"
            :class="{ 'bg-accent font-semibold': activeModel?.reference === model.reference }"
            :aria-current="activeModel?.reference === model.reference ? 'page' : undefined"
            @click.exact.prevent="choose(`model:${model.reference}`)"
          >
            <span class="mt-0.5 size-2 shrink-0 rounded-full bg-primary/70" aria-hidden="true" /><span class="min-w-0"
              ><code class="block break-all">{{ model.name }}</code
              ><span v-if="model.description" class="mt-0.5 block line-clamp-2 text-muted-foreground">{{ model.description }}</span></span
            >
          </component>
        </div>
      </nav>
    </aside>
    <div ref="heading" tabindex="-1" class="min-w-0 scroll-mt-4 px-4 py-4 outline-none sm:px-6">
      <template v-if="active">
        <button type="button" class="mb-3 inline-flex items-center gap-1.5 rounded px-1 text-xs text-muted-foreground hover:text-foreground" @click="choose('')"><ArrowLeft class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.overview") }}</button>
        <OpenApiPublishedOperation :key="active.identityKey" :content="props.content" :document="document" :entry="active" :title="props.title" :version="props.version" :allow-try-it="props.allowTryIt" :allow-export="props.allowExport" :navigate-model="chooseModel" />
      </template>
      <template v-else-if="activeModel">
        <button type="button" class="mb-3 inline-flex items-center gap-1.5 rounded px-1 text-xs text-muted-foreground hover:text-foreground" @click="choose('')"><ArrowLeft class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.overview") }}</button>
        <OpenApiPublishedModel :key="activeModel.reference" :document="document" :model="activeModel" :allow-export="props.allowExport" @navigate-model="chooseModel" @navigate-operation="choose" />
      </template>
      <div v-else class="space-y-3 py-3">
        <p class="eyebrow">OpenAPI {{ document?.openapi }}</p>
        <button v-if="activeTag !== undefined" type="button" class="mb-3 inline-flex items-center gap-1.5 rounded px-1 text-xs text-muted-foreground hover:text-foreground" @click="choose('')"><ArrowLeft class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.overview") }}</button>
        <h2 tabindex="-1" class="text-xl font-semibold break-words outline-none">{{ activeTag ?? props.title }}</h2>
        <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground">
          <span>{{ t("openapiPublication.operationCount", { count: activeTag === undefined ? operations.length : operations.filter((entry) => entry.tags.includes(activeTag!)).length }) }}</span
          ><span v-if="activeTag === undefined">{{ t("openapiPublication.modelCount", { count: models.length }) }}</span
          ><span v-if="infoVersion">{{ t("openapiPublication.apiVersion", { version: infoVersion }) }}</span>
        </div>
        <MarkdownPreview v-if="activeTag !== undefined && typeof tagMetadata?.description === 'string'" :content="tagMetadata.description" class="text-sm" />
        <MarkdownPreview v-else-if="description && activeTag === undefined" :content="description" class="text-sm" />
        <OpenApiDocumentMetadata :info="activeTag === undefined ? document?.info : undefined" :external-docs="activeTag === undefined ? document?.externalDocs : tagMetadata?.externalDocs" />
        <div v-if="servers.length" class="mt-4 rounded-md border border-border bg-muted/20 px-3 py-2">
          <p class="text-xs font-semibold text-muted-foreground">{{ t("openapiPublication.servers") }}</p>
          <ul class="mt-1 space-y-1">
            <li v-for="server in servers" :key="String(server.url)" class="break-all font-mono text-xs text-primary">
              {{ server.url }}<span v-if="server.description" class="ml-2 font-sans text-muted-foreground">{{ server.description }}</span>
            </li>
          </ul>
        </div>
        <p v-if="operations.length" class="text-sm text-muted-foreground">{{ t("openapiPublication.chooseOperation") }}</p>
        <section v-if="catalogue === 'models'" class="grid gap-3 pt-3 sm:grid-cols-2">
          <button
            v-for="model in filteredModels"
            :key="model.reference"
            type="button"
            class="min-w-0 rounded-xl border border-border bg-card p-4 text-left transition-colors hover:border-primary/40 hover:bg-accent/40 focus-visible:outline-2 focus-visible:outline-primary"
            @click="chooseModel(model.reference)"
          >
            <code class="break-all text-sm font-semibold" translate="no">{{ model.name }}</code>
            <p class="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">{{ model.description || model.reference }}</p>
          </button>
        </section>
        <section v-else class="grid gap-3 pt-3 sm:grid-cols-2">
          <div v-for="[tag, entries] in overviewGroups" :key="tag" class="min-w-0 overflow-hidden rounded-xl border border-border bg-card">
            <h3 class="flex items-center justify-between gap-2 border-b border-border bg-muted/20 px-4 py-3 text-sm font-semibold">
              <span class="break-words">{{ tag }}</span
              ><span class="text-xs font-normal text-muted-foreground">{{ entries.length }}</span>
            </h3>
            <ul class="divide-y divide-border/60">
              <li v-for="entry in activeTag === tag ? entries : entries.slice(0, 5)" :key="entry.identityKey">
                <button type="button" class="flex w-full min-w-0 items-center gap-2 px-4 py-3 text-left text-xs transition-colors hover:bg-accent focus-visible:outline-2 focus-visible:outline-primary" @click="choose(entry.identityKey)">
                  <span class="w-12 shrink-0 rounded px-1 py-0.5 text-center font-mono text-[10px] font-semibold" :class="openApiMethodClass(entry.method)" translate="no">{{ entry.method.toUpperCase() }}</span
                  ><span class="min-w-0 break-words">{{ entry.operation.summary || entry.operation.operationId || entry.path }}</span>
                </button>
              </li>
            </ul>
            <component
              v-if="activeTag !== tag && entries.length > 5"
              :is="props.linked ? 'a' : 'button'"
              :href="props.linked ? '#' + encodeURIComponent(`tag:${tag}`) : undefined"
              :type="props.linked ? undefined : 'button'"
              class="block w-full border-t border-border px-4 py-3 text-left text-xs font-medium text-primary hover:bg-accent"
              @click.exact.prevent="choose(`tag:${tag}`)"
              >{{ t("openapiPublication.viewTag", { count: entries.length }) }}</component
            >
          </div>
        </section>
      </div>
      <nav v-if="adjacent.length" :aria-label="t('openapiPublication.reader')" class="mt-6 grid gap-3 border-t border-border pt-5 sm:grid-cols-2">
        <component
          v-for="item in adjacent"
          :key="item.direction"
          :is="props.linked ? 'a' : 'button'"
          :href="props.linked ? '#' + encodeURIComponent(item.page!.key) : undefined"
          :type="props.linked ? undefined : 'button'"
          class="min-w-0 rounded-lg border border-border px-4 py-3 text-left transition-colors hover:border-primary/40 hover:bg-accent focus-visible:outline-2 focus-visible:outline-primary"
          :class="{ 'sm:col-start-2': item.direction === 'nextPage' }"
          @click.exact.prevent="choose(item.page!.key)"
        >
          <span class="block text-xs text-muted-foreground">{{ t(`openapiPublication.${item.direction}`) }}</span>
          <span class="mt-1 block break-all font-mono text-sm font-medium">{{ item.page!.label }}</span>
        </component>
      </nav>
    </div>
  </div>
</template>

<style scoped>
.openapi-catalogue-row {
  content-visibility: auto;
  contain-intrinsic-size: auto 52px;
}
</style>
