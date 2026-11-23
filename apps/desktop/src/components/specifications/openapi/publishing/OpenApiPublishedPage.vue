<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref } from "vue";
import { Check, Clipboard, Download, Moon, Search, Sun } from "@lucide/vue";
import { SpecxMark } from "@/components/brand";
import { Button } from "@/components/ui/button";
import OpenApiDocumentReader from "../documentation/OpenApiDocumentReader.vue";
import { specxApi } from "@/lib/backend/client";
import { ApiError } from "@/lib/backend/types";
import { translate as t } from "@/i18n";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";
import type { PublicOpenApiDocument } from "@/types/api";
import { matchesShortcut } from "@/lib/editor/keyboardShortcuts";
import { normalizeShortcutBindings } from "@/lib/editor/shortcutRegistry";

const props = defineProps<{ id: string }>();
const data = ref<PublicOpenApiDocument>();
const reader = ref<InstanceType<typeof OpenApiDocumentReader>>();
const searchShortcut = normalizeShortcutBindings(undefined).search;
const exportOpen = ref(false);
const ExportDialog = defineAsyncComponent(() => import("../documentation/OpenApiDocumentExportDialog.vue"));
const status = ref<"loading" | "missing" | "failed" | "ready">("loading");
const selection = ref("");
const theme = ref<"light" | "dark">("light");
const { copy, copied, failed: copyFailed } = useClipboardFeedback();
let previousDark = false;
let previousColorScheme = "";
function readHash() {
  try {
    selection.value = decodeURIComponent(location.hash.slice(1));
  } catch {
    selection.value = "";
  }
}
const operation = computed({
  get: () => selection.value,
  set: (value: string) => {
    selection.value = value;
    history.pushState(null, "", location.pathname + (value ? "#" + encodeURIComponent(value) : ""));
  },
});
async function load() {
  status.value = "loading";
  try {
    data.value = await specxApi.getPublicOpenApiDocument(props.id);
    status.value = "ready";
    let preference: string | null = null;
    try {
      preference = localStorage.getItem("specx-document-theme");
    } catch {
      /* Reading remains available when storage is disabled. */
    }
    applyTheme(preference === "light" || preference === "dark" ? preference : data.value.publication.theme);
    document.title = data.value.publication.title + " · SpecX";
  } catch (error) {
    status.value = error instanceof ApiError && error.status === 404 ? "missing" : "failed";
  }
}

function applyTheme(value: "light" | "dark") {
  theme.value = value;
  document.documentElement.classList.toggle("dark", value === "dark");
  document.documentElement.style.colorScheme = value;
}
function toggleTheme() {
  applyTheme(theme.value === "dark" ? "light" : "dark");
  try {
    localStorage.setItem("specx-document-theme", theme.value);
  } catch {
    /* Persistence is optional. */
  }
}

function copyPage() {
  void copy(location.href);
}

function handleSearchShortcut(event: KeyboardEvent) {
  if (!data.value || event.defaultPrevented || !matchesShortcut(event, searchShortcut)) return;
  if (event.target instanceof Element && event.target.closest('[role="dialog"], [role="menu"], [role="listbox"]')) return;
  event.preventDefault();
  void reader.value?.focusSearch();
}

onMounted(() => {
  previousDark = document.documentElement.classList.contains("dark");
  previousColorScheme = document.documentElement.style.colorScheme;
  readHash();
  addEventListener("popstate", readHash);
  addEventListener("hashchange", readHash);
  addEventListener("keydown", handleSearchShortcut);
  void load();
});
onBeforeUnmount(() => {
  removeEventListener("popstate", readHash);
  removeEventListener("hashchange", readHash);
  removeEventListener("keydown", handleSearchShortcut);
  document.documentElement.classList.toggle("dark", previousDark);
  document.documentElement.style.colorScheme = previousColorScheme;
});
</script>

<template>
  <main class="h-full overflow-auto bg-background">
    <a href="#published-document-content" class="sr-only rounded bg-background px-4 py-2 text-sm focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50">{{ t("openapiPublication.skipToContent") }}</a>
    <header class="border-b border-border bg-surface">
      <div class="mx-auto flex max-w-[1600px] flex-wrap items-center justify-between gap-3 px-4 py-4">
        <div class="flex min-w-0 items-center gap-3">
          <SpecxMark :size="26" decorative class="shrink-0" />
          <div class="min-w-0">
            <h1 class="text-sm font-semibold break-words">{{ data?.publication.title ?? t("openapiPublication.reader") }}</h1>
            <p v-if="data" class="text-xs text-muted-foreground">{{ t("specification.revision", { version: data.publication.version }) }}</p>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <Button v-if="data" size="sm" variant="outline" @click="reader?.focusSearch()"><Search class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.searchDocs") }}</Button>
          <Button v-if="data" size="sm" variant="outline" :aria-label="t(theme === 'light' ? 'openapiPublication.useDarkTheme' : 'openapiPublication.useLightTheme')" @click="toggleTheme"
            ><Moon v-if="theme === 'light'" class="size-4" aria-hidden="true" /><Sun v-else class="size-4" aria-hidden="true"
          /></Button>
          <Button size="sm" variant="outline" @click="copyPage"><Check v-if="copied" class="size-3.5" aria-hidden="true" /><Clipboard v-else class="size-3.5" aria-hidden="true" />{{ copied ? t("openapiPublication.copiedPage") : t("openapiPublication.copyPage") }}</Button>
          <Button v-if="data?.publication.showDownload" size="sm" variant="outline" @click="exportOpen = true"><Download class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.export") }}</Button>
        </div>
      </div>
      <p v-if="data?.publication.description" class="mx-auto max-w-[1600px] px-4 pb-4 text-sm leading-6 text-muted-foreground sm:px-6">{{ data.publication.description }}</p>
    </header>
    <p v-if="copyFailed" role="status" class="mx-auto max-w-[1600px] px-4 py-2 text-xs text-destructive">{{ t("specification.openapiCopyFailed") }}</p>
    <OpenApiDocumentReader
      v-if="data"
      ref="reader"
      id="published-document-content"
      v-model:operation="operation"
      :content="data.content"
      :title="data.publication.title"
      :version="data.publication.version"
      :allow-try-it="data.publication.allowTryIt"
      :allow-export="data.publication.showDownload"
      linked
      class="mx-auto min-h-[calc(100%-5rem)] max-w-[1600px]"
    />
    <ExportDialog v-if="data?.publication.showDownload && exportOpen" :content="data.content" :title="data.publication.title" :selection="selection" @close="exportOpen = false" />
    <div v-if="!data" class="mx-auto max-w-lg space-y-3 px-5 py-12 text-center">
      <p v-if="status === 'loading'" role="status" class="text-sm text-muted-foreground">{{ t("common.loading") }}</p>
      <template v-else>
        <p role="alert" class="text-sm">{{ t(status === "missing" ? "openapiPublication.missing" : "openapiPublication.loadFailed") }}</p>
        <Button v-if="status === 'failed'" size="sm" variant="outline" @click="load">{{ t("common.retry") }}</Button>
      </template>
    </div>
  </main>
</template>
