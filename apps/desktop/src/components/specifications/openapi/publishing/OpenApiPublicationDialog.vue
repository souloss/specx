<script setup lang="ts">
import AppSelect from "@/components/ui/AppSelect.vue";
import AppAppearanceScope from "@/components/ui/AppAppearanceScope.vue";
import { computed, onMounted, ref, watch } from "vue";
import { ArrowLeft, Clipboard, ExternalLink, LoaderCircle, Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import OpenApiPublicEnvironmentEditor from "./OpenApiPublicEnvironmentEditor.vue";
import { validPublicEnvironments, withPublicEnvironments } from "@/lib/specifications/openapi/environments/openapiPublicEnvironments";
import type { OpenApiEnvironment, PublicOpenApiEnvironment } from "@/types/api";
import OpenApiDocumentReader from "../documentation/OpenApiDocumentReader.vue";
import { specxApi } from "@/lib/backend/client";
import { ApiError } from "@/lib/backend/types";
import { i18n, translate as t } from "@/i18n";
import { formatDate } from "@/lib/core";
import { openApiPublicationUrl } from "@/lib/specifications/openapi/publishing/openapiPublication";
import type { OpenApiPublication, OpenApiPublicationPreview, SpecificationRevisionResponse } from "@/types/api";
import type { Specification } from "@/types/domain";

const props = defineProps<{ workspaceId: string; specification: Specification; environments?: OpenApiEnvironment[] }>();
const emit = defineEmits<{ close: [] }>();
const mode = ref<"manage" | "review" | "revoke">("manage");
const items = ref<OpenApiPublication[]>([]);
const nextPage = ref<number | null>(null);
const revisions = ref<SpecificationRevisionResponse[]>([]);
const nextRevisionPage = ref<number | null>(null);
const revokeTarget = ref<OpenApiPublication>();
const version = ref(props.specification.version);
const title = ref(props.specification.title.slice(0, 120));
const description = ref("");
const theme = ref<"light" | "dark">("light");
const showDownload = ref(true);
const allowTryIt = ref(true);
const publicEnvironments = ref<PublicOpenApiEnvironment[]>([]);
const environmentsValid = computed(() => validPublicEnvironments(publicEnvironments.value));
const previewContent = computed(() => (preview.value ? withPublicEnvironments(preview.value.content, publicEnvironments.value) : ""));
const preview = ref<OpenApiPublicationPreview>();
const busy = ref(false);
const mutating = ref(false);
const failed = ref(false);
const error = ref("");
const notice = ref("");
const operation = ref("");
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
const publishedVersion = computed(() => items.value.some((item) => item.version === version.value));
let previewGeneration = 0;

function link(id: string) {
  return openApiPublicationUrl(id);
}
function failure(cause: unknown, fallback: string) {
  return cause instanceof ApiError ? cause.message : fallback;
}

async function load(offset = 0) {
  busy.value = true;
  failed.value = false;
  error.value = "";
  try {
    const page = await specxApi.listOpenApiPublications(props.workspaceId, props.specification.id, offset);
    items.value = offset === 0 ? page.items : [...items.value, ...page.items];
    nextPage.value = page.nextOffset;
  } catch (cause) {
    failed.value = offset === 0;
    error.value = failure(cause, t("openapiPublication.loadFailed"));
  } finally {
    busy.value = false;
  }
}

async function loadRevisions(offset = 0) {
  try {
    const page = await specxApi.listSpecificationRevisions(props.workspaceId, props.specification.id, 20, offset);
    revisions.value = offset === 0 ? page.items : [...revisions.value, ...page.items];
    nextRevisionPage.value = page.nextOffset ?? null;
  } catch (cause) {
    error.value = failure(cause, t("openapiPublication.loadFailed"));
  }
}

async function review() {
  mode.value = "review";
  notice.value = "";
  version.value = props.specification.version;
  await Promise.all([loadRevisions(), loadPreview()]);
}

async function loadPreview() {
  if (mutating.value) return;
  const generation = ++previewGeneration;
  preview.value = undefined;
  operation.value = "";
  error.value = "";
  busy.value = true;
  try {
    const result = await specxApi.previewOpenApiPublication(props.workspaceId, props.specification.id, version.value);
    if (generation === previewGeneration) preview.value = result;
  } catch (cause) {
    if (generation === previewGeneration) error.value = cause instanceof ApiError && cause.status === 422 ? t("openapiPublication.contentRejected") : failure(cause, t("openapiPublication.loadFailed"));
  } finally {
    if (generation === previewGeneration) busy.value = false;
  }
}

async function publish() {
  if (busy.value || mutating.value || !preview.value || !title.value.trim() || publishedVersion.value || !environmentsValid.value) return;
  mutating.value = true;
  busy.value = true;
  error.value = "";
  try {
    const item = await specxApi.publishOpenApi(props.workspaceId, props.specification.id, {
      title: title.value.trim(),
      description: description.value.trim(),
      version: version.value,
      theme: theme.value,
      showDownload: showDownload.value,
      allowTryIt: allowTryIt.value,
      digest: preview.value.digest,
      ...(publicEnvironments.value.length ? { environments: publicEnvironments.value } : {}),
    });
    items.value = [item, ...items.value].sort((a, b) => b.version - a.version);
    if (nextPage.value !== null) nextPage.value += 1;
    notice.value = t("openapiPublication.published");
    mode.value = "manage";
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 409) {
      await load();
      mode.value = "manage";
      error.value = t("openapiPublication.duplicate");
    } else error.value = cause instanceof ApiError && cause.status === 422 ? t("openapiPublication.contentRejected") : failure(cause, t("openapiPublication.publishFailed"));
  } finally {
    mutating.value = false;
    busy.value = false;
  }
}

async function revoke() {
  if (busy.value || mutating.value || !revokeTarget.value) return;
  mutating.value = true;
  busy.value = true;
  error.value = "";
  try {
    const id = revokeTarget.value.id;
    await specxApi.revokeOpenApiPublication(props.workspaceId, props.specification.id, id);
    items.value = items.value.filter((item) => item.id !== id);
    if (nextPage.value !== null) nextPage.value -= 1;
    mode.value = "manage";
    notice.value = t("openapiPublication.revoked");
  } catch (cause) {
    error.value = failure(cause, t("openapiPublication.revokeFailed"));
  } finally {
    mutating.value = false;
    busy.value = false;
  }
}

async function copy(id: string) {
  try {
    await navigator.clipboard.writeText(link(id));
    notice.value = t("openapiPublication.copied");
  } catch {
    error.value = t("specification.openapiCopyFailed");
  }
}

function back() {
  if (mutating.value) return;
  error.value = "";
  mode.value = "manage";
}
watch(version, () => {
  if (mode.value === "review") void loadPreview();
});
onMounted(() => void load());
</script>

<template>
  <Dialog :open="true" @update:open="(open) => !open && emit('close')">
    <DialogScrollContent
      class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0"
      :class="mode === 'review' ? 'sm:max-w-5xl' : 'sm:max-w-2xl'"
      :show-close-button="!mutating"
      @interact-outside="(event) => mutating && event.preventDefault()"
      @escape-key-down="(event) => mutating && event.preventDefault()"
    >
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle>{{ t(mode === "review" ? "openapiPublication.review" : mode === "revoke" ? "openapiPublication.revoke" : "openapiPublication.manage") }}</DialogTitle>
        <DialogDescription>{{ t(mode === "review" ? "openapiPublication.reviewHelp" : mode === "revoke" ? "openapiPublication.revokeHelp" : "openapiPublication.manageHelp") }}</DialogDescription>
      </DialogHeader>
      <div class="min-h-0 overflow-auto overscroll-contain px-5 py-4">
        <p v-if="error" role="alert" class="mb-3 text-sm text-destructive">{{ error }}</p>
        <p v-if="notice" role="status" class="mb-3 text-sm text-muted-foreground">{{ notice }}</p>
        <template v-if="mode === 'manage'">
          <p v-if="busy && !items.length" role="status" class="text-sm text-muted-foreground">{{ t("common.loading") }}</p>
          <Button v-else-if="failed" size="sm" variant="outline" @click="load()">{{ t("common.retry") }}</Button>
          <p v-else-if="!items.length" class="rounded-md border border-dashed border-border px-4 py-6 text-center text-sm text-muted-foreground">{{ t("openapiPublication.empty") }}</p>
          <ul v-else class="divide-y divide-border rounded-md border border-border">
            <li v-for="item in items" :key="item.id" class="flex flex-wrap items-center justify-between gap-3 px-3 py-3">
              <div class="min-w-0">
                <p class="text-sm font-medium break-words">{{ item.title }}</p>
                <p class="text-xs text-muted-foreground">{{ t("specification.revision", { version: item.version }) }} · {{ formatDate(item.createdAt, locale) }}</p>
              </div>
              <div class="flex flex-wrap items-center gap-1.5">
                <Button as="a" :href="link(item.id)" target="_blank" rel="noopener noreferrer" size="sm" variant="outline"><ExternalLink class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.open") }}</Button>
                <Button size="sm" variant="outline" @click="copy(item.id)"><Clipboard class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.copy") }}</Button>
                <Button
                  size="sm"
                  variant="ghost"
                  :disabled="busy"
                  @click="
                    revokeTarget = item;
                    notice = '';
                    error = '';
                    mode = 'revoke';
                  "
                  ><Trash2 class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.revoke") }}</Button
                >
              </div>
            </li>
          </ul>
          <Button v-if="nextPage !== null" size="sm" variant="ghost" class="mt-2" :disabled="busy" @click="load(nextPage)">{{ t("openapiPublication.more") }}</Button>
        </template>
        <div v-else-if="mode === 'review'" class="space-y-3">
          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <label class="grid gap-1.5 text-sm">{{ t("openapiPublication.title") }}<input v-model="title" name="openapi-publication-title" autocomplete="off" class="field-control w-full" maxlength="120" :disabled="mutating" /></label>
            <label class="grid gap-1.5 text-sm">
              {{ t("openapiPublication.version") }}
              <AppSelect v-model.number="version" size="form" name="openapi-publication-revision" class="field-control w-full" :disabled="mutating">
                <option v-if="!revisions.some((item) => item.version === specification.version)" :value="specification.version">{{ t("specification.revision", { version: specification.version }) }}</option>
                <option v-for="item in revisions" :key="item.version" :value="item.version">{{ t("specification.revision", { version: item.version }) }}</option>
              </AppSelect>
            </label>
          </div>
          <label class="grid gap-1.5 text-sm">
            {{ t("openapiPublication.description") }}
            <textarea v-model="description" name="openapi-publication-description" class="field-control min-h-20 w-full resize-y" maxlength="500" :disabled="mutating" />
          </label>
          <div class="grid gap-3 sm:grid-cols-2">
            <fieldset class="grid gap-1.5 text-sm" :disabled="mutating">
              <legend>{{ t("openapiPublication.theme") }}</legend>
              <div role="group" :aria-label="t('openapiPublication.theme')" class="inline-flex w-fit rounded-md border border-border p-0.5">
                <button type="button" class="rounded px-3 py-1.5 text-sm transition-colors" :class="theme === 'light' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'" :aria-pressed="theme === 'light'" @click="theme = 'light'">{{ t("openapiPublication.light") }}</button>
                <button type="button" class="rounded px-3 py-1.5 text-sm transition-colors" :class="theme === 'dark' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'" :aria-pressed="theme === 'dark'" @click="theme = 'dark'">{{ t("openapiPublication.dark") }}</button>
              </div>
            </fieldset>
            <label class="flex items-center gap-2 self-end pb-1 text-sm">
              <input v-model="showDownload" name="openapi-publication-download" type="checkbox" class="size-4 accent-primary" :disabled="mutating" />
              {{ t("openapiPublication.allowDownload") }}
            </label>
            <label class="flex items-center gap-2 self-end pb-1 text-sm">
              <input v-model="allowTryIt" name="openapi-publication-try-it" type="checkbox" class="size-4 accent-primary" :disabled="mutating" />
              {{ t("openapiPublication.allowTryIt") }}
            </label>
          </div>
          <OpenApiPublicEnvironmentEditor v-model="publicEnvironments" :sources="props.environments" :disabled="mutating" :valid="environmentsValid" />
          <Button v-if="nextRevisionPage !== null" size="sm" variant="ghost" :disabled="mutating" @click="loadRevisions(nextRevisionPage)">{{ t("openapiPublication.more") }}</Button>
          <p v-if="publishedVersion" class="text-sm text-muted-foreground">{{ t("openapiPublication.duplicate") }}</p>
          <p v-if="busy && !preview" role="status" class="text-sm text-muted-foreground">{{ t("common.loading") }}</p>
          <Button v-else-if="!preview && error" size="sm" variant="outline" @click="loadPreview">{{ t("common.retry") }}</Button>
          <AppAppearanceScope v-if="preview" :theme="theme" class="overflow-hidden rounded-md border border-border">
            <OpenApiDocumentReader v-model:operation="operation" :content="previewContent" :title="title" :version="version" :allow-try-it="allowTryIt" :allow-export="showDownload" />
          </AppAppearanceScope>
        </div>
        <div v-else-if="revokeTarget">
          <p class="text-sm font-semibold break-words">{{ revokeTarget.title }}</p>
          <p class="text-xs text-muted-foreground">{{ t("specification.revision", { version: revokeTarget.version }) }}</p>
        </div>
      </div>
      <DialogFooter class="m-0 rounded-b-lg border-t border-border px-5 py-4">
        <template v-if="mode === 'manage'">
          <Button variant="outline" @click="emit('close')">{{ t("common.close") }}</Button>
          <Button :disabled="busy || failed" @click="review"><Plus class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.new") }}</Button>
        </template>
        <template v-else>
          <Button variant="outline" :disabled="busy" @click="back"><ArrowLeft class="size-3.5" aria-hidden="true" />{{ t("common.cancel") }}</Button>
          <Button v-if="mode === 'review'" :disabled="busy || !preview || !title.trim() || publishedVersion || !environmentsValid" :aria-busy="mutating" @click="publish"
            ><LoaderCircle v-if="mutating" class="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />{{ t("openapiPublication.publish") }}</Button
          >
          <Button v-else variant="destructive" :disabled="busy" :aria-busy="mutating" @click="revoke"><LoaderCircle v-if="mutating" class="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />{{ t("openapiPublication.revoke") }}</Button>
        </template>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
