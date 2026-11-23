<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Clock3, Eye, RotateCcw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import MarkdownPreview from "./markdown/MarkdownPreview.vue";
import OpenApiPreview from "./openapi/OpenApiPreview.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Specification } from "@/types/domain";
import { formatDate } from "@/lib/core";
import { i18n, translate } from "@/i18n";
import { compareOpenApiDocuments } from "@/lib/specifications/openapi/design/openapiDiff";

const props = defineProps<{ specification: Specification; managed?: boolean }>();
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const selectedVersion = ref<number | null>(null);
const restoreVersion = ref<number | null>(null);
const error = ref("");
const busy = ref(false);
const confirmationText = ref<HTMLElement | null>(null);
const restoreButton = ref<InstanceType<typeof Button> | null>(null);
const locale = computed(() => i18n.global.locale.value as "zh-CN" | "en");
const revisions = computed(() => workspace.specificationRevisions(props.specification.id));
const selected = computed(() => revisions.value.find((item) => item.version === selectedVersion.value));
const loading = computed(() => workspace.specificationRevisionsLoading(props.specification.id));
const hasDraft = computed(() => workspace.hasUnsavedSpecification(props.specification.id));
const openApiDiff = computed(() => (props.specification.kind === "openapi" && selected.value && selected.value.version !== props.specification.version ? compareOpenApiDocuments(selected.value.content, props.specification.content) : null));

function diffMessage(message: string): string {
  const labels: Record<string, string> = {
    operation_removed: t("specification.openapiDiffOperationRemoved"),
    operation_added: t("specification.openapiDiffOperationAdded"),
    required_parameter_added: t("specification.openapiDiffRequiredParameterAdded"),
    required_parameter_removed: t("specification.openapiDiffRequiredParameterRemoved"),
    parameter_type_changed: t("specification.openapiDiffParameterTypeChanged"),
    required_request_body_added: t("specification.openapiDiffRequiredRequestBodyAdded"),
    request_body_now_required: t("specification.openapiDiffRequestBodyNowRequired"),
    request_body_type_changed: t("specification.openapiDiffRequestBodyTypeChanged"),
    response_removed: t("specification.openapiDiffResponseRemoved"),
    response_added: t("specification.openapiDiffResponseAdded"),
    response_type_changed: t("specification.openapiDiffResponseTypeChanged"),
    required_response_property_removed: t("specification.openapiDiffRequiredResponsePropertyRemoved"),
    required_response_property_added: t("specification.openapiDiffRequiredResponsePropertyAdded"),
    response_property_added: t("specification.openapiDiffResponsePropertyAdded"),
    schema_removed: t("specification.openapiDiffSchemaRemoved"),
    schema_added: t("specification.openapiDiffSchemaAdded"),
    schema_type_changed: t("specification.openapiDiffSchemaTypeChanged"),
  };
  return labels[message] ?? message;
}

watch(open, async (value) => {
  if (!value) return;
  restoreVersion.value = null;
  error.value = "";
  try {
    const loaded = await workspace.loadSpecificationRevisions(props.specification.id);
    selectedVersion.value = loaded.find((revision) => revision.version === props.specification.version)?.version ?? loaded[0]?.version ?? null;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.historyLoadFailed");
  }
});

async function confirmRestore(version: number) {
  restoreVersion.value = version;
  await nextTick();
  confirmationText.value?.focus();
}

async function cancelRestore() {
  restoreVersion.value = null;
  await nextTick();
  const button = restoreButton.value?.$el;
  if (button instanceof HTMLElement) button.focus();
}

async function restore() {
  if (!restoreVersion.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.restoreSpecificationRevision(props.specification.id, restoreVersion.value);
    selectedVersion.value = null;
    restoreVersion.value = null;
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.restoreFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent :class="restoreVersion === null ? 'sm:max-w-5xl sm:grid-rows-[auto_minmax(0,1fr)_auto]' : 'sm:max-w-sm'">
      <DialogHeader
        ><DialogTitle class="flex items-center gap-2"><Clock3 class="h-4 w-4" />{{ restoreVersion === null ? t("specification.revisionHistory") : t("specification.restoreConfirmTitle", { version: restoreVersion }) }}</DialogTitle></DialogHeader
      >
      <div v-if="restoreVersion === null" class="grid min-h-0 gap-4 sm:grid-cols-[220px_minmax(0,1fr)]">
        <nav class="min-h-0 overflow-y-auto overscroll-contain rounded-md border border-border p-1" :aria-label="t('specification.revisionHistory')">
          <p v-if="loading" class="p-3 text-xs text-muted-foreground">{{ t("common.loading") }}</p>
          <p v-else-if="!revisions.length" class="p-3 text-xs text-muted-foreground">{{ t("specification.historyEmpty") }}</p>
          <button
            v-for="revision in revisions"
            :key="revision.version"
            type="button"
            class="flex w-full flex-col gap-1 rounded px-3 py-2 text-left text-xs hover:bg-accent"
            :class="selectedVersion === revision.version ? 'bg-accent text-foreground' : 'text-muted-foreground'"
            @click="selectedVersion = revision.version"
          >
            <span class="flex items-center justify-between gap-2 font-medium"
              ><span>{{ t("specification.revision", { version: revision.version }) }}</span
              ><span v-if="revision.version === specification.version" class="text-[10px]">{{ t("specification.current") }}</span></span
            >
            <span>{{ t("specification.revisionBy", { actor: revision.createdBy, date: formatDate(revision.createdAt, locale) }) }}</span>
          </button>
        </nav>
        <div class="min-h-0 overflow-y-auto overscroll-contain rounded-md border border-border p-4">
          <div v-if="selected" class="space-y-3">
            <div class="flex flex-wrap items-center justify-between gap-2 border-b border-border pb-3">
              <div>
                <strong>{{ t("specification.revision", { version: selected.version }) }}</strong>
                <p class="text-xs text-muted-foreground">{{ t("specification.revisionBy", { actor: selected.createdBy, date: formatDate(selected.createdAt, locale) }) }}</p>
              </div>
              <Button
                v-if="selected.version !== specification.version"
                ref="restoreButton"
                size="sm"
                variant="outline"
                :disabled="hasDraft || managed"
                :title="managed ? t('specification.managedReadOnlyDescription') : hasDraft ? t('specification.saveBeforeRestore') : undefined"
                @click="confirmRestore(selected.version)"
                ><RotateCcw class="h-3.5 w-3.5" />{{ t("specification.restoreVersion") }}</Button
              >
            </div>
            <p v-if="hasDraft" class="rounded-md border border-amber-300 bg-amber-50 p-2 text-xs text-amber-950 dark:border-amber-700 dark:bg-amber-950/30 dark:text-amber-100">{{ t("specification.saveBeforeRestore") }}</p>
            <p v-else-if="managed" class="rounded-md border border-primary/30 bg-primary/5 p-2 text-xs text-muted-foreground">{{ t("specification.managedReadOnlyDescription") }} {{ t("specification.managedUnbindDescription") }}</p>
            <section v-if="openApiDiff" class="rounded-md border border-border bg-muted/20 p-3" :aria-label="t('specification.openapiDiff')">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <strong class="text-sm">{{ t("specification.openapiDiff") }}</strong>
                <span class="rounded-full px-2 py-0.5 text-xs" :class="openApiDiff.breakingCount ? 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-300' : 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'">{{
                  t("specification.openapiDiffImpact", { impact: openApiDiff.impact })
                }}</span>
              </div>
              <div class="mt-2 flex flex-wrap gap-3 text-xs text-muted-foreground">
                <span>{{ t("specification.openapiDiffBreaking", { count: openApiDiff.breakingCount }) }}</span>
                <span>{{ t("specification.openapiDiffSafe", { count: openApiDiff.safeCount }) }}</span>
              </div>
              <ul v-if="openApiDiff.changes.length" class="mt-2 space-y-1 text-xs">
                <li v-for="change in openApiDiff.changes" :key="`${change.message}:${change.path}`" :class="change.severity === 'breaking' ? 'text-rose-700 dark:text-rose-300' : 'text-emerald-700 dark:text-emerald-300'">
                  <code>{{ change.path }}</code> · {{ diffMessage(change.message) }}
                </li>
              </ul>
              <p v-else class="mt-2 text-xs text-muted-foreground">{{ t("specification.openapiDiffNoChanges") }}</p>
            </section>
            <div class="grid gap-3 lg:grid-cols-2">
              <div>
                <div class="mb-2 flex items-center gap-1 text-xs font-medium text-muted-foreground"><Eye class="h-3.5 w-3.5" />{{ t("specification.revision", { version: selected.version }) }}</div>
                <OpenApiPreview v-if="specification.kind === 'openapi'" :content="selected.content" />
                <MarkdownPreview v-else :content="selected.content" />
              </div>
              <div v-if="selected.version !== specification.version">
                <div class="mb-2 text-xs font-medium text-muted-foreground">{{ t("specification.current") }}</div>
                <OpenApiPreview v-if="specification.kind === 'openapi'" :content="specification.content" />
                <MarkdownPreview v-else :content="specification.content" />
              </div>
            </div>
          </div>
          <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t("specification.historyEmpty") }}</p>
        </div>
      </div>
      <p v-else ref="confirmationText" tabindex="-1" class="text-sm text-muted-foreground">{{ t("specification.restoreConfirmDescription") }}</p>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <DialogFooter v-if="restoreVersion === null"
        ><Button variant="outline" @click="open = false">{{ t("common.close") }}</Button></DialogFooter
      >
      <DialogFooter v-else
        ><Button variant="outline" :disabled="busy" @click="cancelRestore">{{ t("common.cancel") }}</Button
        ><Button :disabled="busy" @click="restore"><RotateCcw class="h-3.5 w-3.5" />{{ busy ? t("common.loading") : t("common.restore") }}</Button></DialogFooter
      >
    </DialogContent>
  </Dialog>
</template>
