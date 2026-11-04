<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Download, ExternalLink, Loader2, RefreshCw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useAppUpdateStore } from "@/stores/appUpdateStore";
import { useNavigationStore } from "@/stores/navigationStore";
import { fetchAppChangelog } from "@/lib/backend/appRelease";
import { buildInfo, bundledChangelog, releasesUrl, type ChangelogRelease } from "@/lib/app/releases";
import { i18n, translate as t } from "@/i18n";
import { pendingWrites } from "@/lib/backend/writeActivity";

const updates = useAppUpdateStore();
const navigation = useNavigationStore();
const confirmInstall = ref(false);
const loading = ref(false);
const remoteFailed = ref(false);
const remote = ref<{ locale: string; releases: ChangelogRelease[] }>();
const visibleCount = ref(5);
const releases = computed(() => (remote.value?.locale === i18n.global.locale.value ? remote.value.releases : bundledChangelog(i18n.global.locale.value).releases));
const releasePage = releasesUrl();
let request = 0;

async function loadChangelog() {
  const id = ++request;
  const locale = i18n.global.locale.value;
  visibleCount.value = 5;
  if (!updates.desktop || !updates.configured) return;
  loading.value = true;
  remoteFailed.value = false;
  try {
    const data = await fetchAppChangelog(locale);
    if (id === request) remote.value = { locale, releases: data.releases };
  } catch {
    if (id === request) remoteFailed.value = true;
  } finally {
    if (id === request) loading.value = false;
  }
}

function releaseDate(value: string) {
  return new Intl.DateTimeFormat(i18n.global.locale.value, { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" }).format(new Date(value));
}

async function install() {
  if (pendingWrites.value || navigation.busy || navigation.naming) return;
  confirmInstall.value = false;
  await updates.install();
}

onMounted(async () => {
  await updates.initialize();
  await loadChangelog();
});
watch(() => i18n.global.locale.value, loadChangelog);
</script>

<template>
  <section class="space-y-5 py-2">
    <div class="space-y-2">
      <h2 class="text-lg font-medium">
        specX <span class="font-mono text-sm text-muted-foreground">v{{ updates.version }}</span>
      </h2>
      <p class="text-sm text-muted-foreground">{{ t("settings.aboutDescription") }}</p>
      <p class="text-xs text-muted-foreground">
        {{ updates.desktop ? t("updates.desktopRuntime") : t("updates.webRuntime") }}<span v-if="buildInfo.commit"> · {{ buildInfo.commit }}</span>
      </p>
    </div>
    <div class="space-y-3 rounded-lg border border-border p-4">
      <h3 class="text-sm font-medium">{{ t("updates.title") }}</h3>
      <p v-if="!updates.desktop" class="text-sm text-muted-foreground">{{ t("updates.webHint") }}</p>
      <p v-else-if="updates.initialized && !updates.configured" class="text-sm text-muted-foreground">{{ t("updates.notConfigured") }}</p>
      <template v-else-if="updates.desktop">
        <label class="flex items-center gap-2 text-sm"><input v-model="updates.autoCheck" type="checkbox" />{{ t("updates.autoCheck") }}</label>
        <p v-if="updates.available" role="status" class="text-sm">{{ t("updates.available", { current: updates.version, latest: updates.latestVersion }) }}</p>
        <p v-else-if="updates.checked && !updates.errorKey" role="status" class="text-sm text-muted-foreground">{{ t("updates.upToDate", { version: updates.version }) }}</p>
        <p v-if="updates.phase === 'downloaded'" class="text-sm">{{ t("updates.downloaded") }}</p>
        <div v-if="updates.phase === 'downloading'" class="space-y-2" role="status">
          <p class="text-sm">{{ updates.progress === undefined ? t("updates.downloading") : t("updates.progress", { percent: updates.progress }) }}</p>
          <progress v-if="updates.progress !== undefined" class="h-2 w-full accent-primary" :value="updates.progress" max="100" :aria-label="t('updates.downloading')" />
        </div>
        <p v-if="updates.errorKey" role="alert" class="text-sm text-destructive">{{ t(updates.errorKey) }}</p>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" :disabled="updates.busy || ['downloaded', 'restart'].includes(updates.phase)" @click="updates.check">
            <Loader2 v-if="updates.phase === 'checking'" class="h-3.5 w-3.5 animate-spin" /><RefreshCw v-else class="h-3.5 w-3.5" />{{ t("updates.check") }}
          </Button>
          <Button v-if="updates.phase === 'available'" size="sm" @click="updates.download"><Download class="h-3.5 w-3.5" />{{ t("updates.download") }}</Button>
          <Button v-if="updates.phase === 'downloaded'" size="sm" @click="confirmInstall = true">{{ t("updates.install") }}</Button>
          <Button v-if="updates.phase === 'restart'" size="sm" @click="updates.restart">{{ t("updates.restart") }}</Button>
          <p v-if="updates.phase === 'installing'" role="status" class="text-sm">{{ t("updates.installing") }}</p>
        </div>
      </template>
      <a v-if="releasePage" :href="releasePage" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 text-xs text-primary underline underline-offset-2"><ExternalLink class="h-3 w-3" />{{ t("updates.releasePage") }}</a>
    </div>
    <div class="space-y-3 rounded-lg border border-border p-4">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="text-sm font-medium">{{ t("updates.changelog") }}</h3>
        <Button v-if="updates.desktop && updates.configured" size="sm" variant="outline" :disabled="loading" @click="loadChangelog"><RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />{{ t("updates.refreshNotes") }}</Button>
      </div>
      <p v-if="remoteFailed" role="status" class="text-xs text-muted-foreground">{{ t("updates.notesFallback") }}</p>
      <p v-if="!releases.length && !loading" class="text-sm text-muted-foreground">{{ t("updates.noReleases") }}</p>
      <p v-if="loading" role="status" class="text-sm text-muted-foreground">{{ t("updates.loadingNotes") }}</p>
      <details v-for="release in releases.slice(0, visibleCount)" :key="release.tag" class="rounded-md border border-border p-3">
        <summary class="cursor-pointer text-sm font-medium">
          {{ release.tag }} <span class="ml-2 text-xs font-normal text-muted-foreground">{{ releaseDate(release.date) }}</span>
        </summary>
        <div v-for="(section, index) in release.sections" :key="index" class="mt-3 space-y-2">
          <h4 class="text-xs font-medium">{{ section.title }}</h4>
          <ul class="list-disc space-y-1 pl-4 text-xs leading-relaxed text-muted-foreground">
            <li v-for="(item, itemIndex) in section.items" :key="itemIndex">
              {{ item.title }}<span v-if="item.desc"> — {{ item.desc }}</span>
            </li>
          </ul>
        </div>
        <a v-if="releasesUrl(release.tag)" :href="releasesUrl(release.tag)" target="_blank" rel="noopener noreferrer" class="mt-3 inline-block text-xs text-primary underline">{{ t("updates.releasePage") }}</a>
      </details>
      <Button v-if="visibleCount < releases.length" variant="outline" size="sm" @click="visibleCount += 5">{{ t("updates.moreNotes") }}</Button>
    </div>
    <Dialog v-model:open="confirmInstall"
      ><DialogContent
        ><DialogHeader
          ><DialogTitle>{{ t("updates.install") }}</DialogTitle
          ><DialogDescription>{{ t("updates.confirmInstall") }}</DialogDescription></DialogHeader
        >
        <p v-if="pendingWrites || navigation.busy || navigation.naming" role="alert" class="text-sm text-destructive">{{ t("updates.pendingWork") }}</p>
        <DialogFooter
          ><Button variant="outline" @click="confirmInstall = false">{{ t("common.cancel") }}</Button
          ><Button :disabled="!!pendingWrites || navigation.busy || !!navigation.naming" @click="install">{{ t("updates.install") }}</Button></DialogFooter
        >
      </DialogContent></Dialog
    >
  </section>
</template>
