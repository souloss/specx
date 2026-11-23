<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { ArrowLeft, ArrowUpRight, Braces, Check, FileCode2, Layers3, LoaderCircle, Plus, Search, Trash2 } from "@lucide/vue";
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from "reka-ui";
import { parse } from "yaml";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";
import OpenApiSchemaDesignEditor from "./OpenApiSchemaDesignEditor.vue";
import OpenApiCodeEditor from "../shared/OpenApiCodeEditor.vue";
import { formatOpenApiContent, isRecord, parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import { deleteOpenApiModel, getOpenApiModels, getOpenApiModelUsages, getOpenApiModelDependencies, openApiModelDeletionBlockers, updateOpenApiModel, validateOpenApiModelName, type OpenApiModelChange } from "@/lib/specifications/openapi/contract/openapiModels";
import { openApiPointer, openApiPointerParts } from "@/lib/specifications/openapi/contract/openapiReferences";
import { validateOpenApiSchemaInWorker } from "@/lib/specifications/openapi/contract/openapiSchemaValidationClient";
import { translate } from "@/i18n";

const props = defineProps<{
  content: string;
  modelRef?: string;
  specificationTitle: string;
  editable: boolean;
  draftScope: string;
  saveDocument: (content: string, change?: OpenApiModelChange) => Promise<boolean>;
  navigateModel: (reference?: string) => void;
  navigateOperation: (key: string, title: string) => void;
}>();
const t = translate;
const document = computed(() => parseOpenApiContent(props.content));
const models = computed(() => getOpenApiModels(document.value));
const selected = computed(() => models.value.find((model) => model.reference === props.modelRef));
const query = ref("");
const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase();
  return models.value.filter((model) => `${model.name} ${model.description} ${JSON.stringify(model.schema)}`.toLocaleLowerCase().includes(needle));
});
const dependencies = computed(() => (document.value && selected.value ? getOpenApiModelDependencies(document.value, selected.value.reference) : []));
const usages = computed(() => (document.value && selected.value ? getOpenApiModelUsages(document.value, selected.value.reference) : []));
const blockers = computed(() => (document.value && selected.value ? openApiModelDeletionBlockers(document.value, selected.value.reference) : []));
const mode = ref("preview");
const editor = ref("fields");
const name = ref("");
const source = ref("");
const format = ref<"json" | "yaml">("json");
const baseContext = ref("");
const initialName = ref("");
const initialSource = ref("");
const busy = ref(false);
let disposed = false;
onBeforeUnmount(() => {
  disposed = true;
});
const error = ref("");
const saved = ref(false);
const storageFailed = ref(false);
const createOpen = ref(false);
const deleteOpen = ref(false);
const createName = ref("");
const createDescription = ref("");
const context = computed(() => JSON.stringify({ openapi: document.value?.openapi, components: document.value?.components, definitions: document.value?.definitions }));
const draftKey = computed(() => `specx-openapi-model-draft-v1:${JSON.stringify([props.draftScope, props.modelRef])}`);
const dirty = computed(() => name.value !== initialName.value || source.value !== initialSource.value);
const conflict = computed(() => dirty.value && context.value !== baseContext.value);
const schemaValue = computed(() => {
  try {
    const value: unknown = parse(source.value);
    return isRecord(value) || typeof value === "boolean" ? value : undefined;
  } catch {
    return undefined;
  }
});
const validation = computed(() => (document.value ? validateOpenApiModelName(document.value, name.value, props.modelRef) : null));
const createValidation = computed(() => (document.value && createName.value ? validateOpenApiModelName(document.value, createName.value) : null));
const schemaInvalid = computed(() => schemaValue.value === undefined || (typeof schemaValue.value === "boolean" && !(typeof document.value?.openapi === "string" && document.value.openapi.startsWith("3.1"))));

function clearDraft(key = draftKey.value) {
  try {
    localStorage.removeItem(key);
  } catch {
    /* The visible draft is retained if storage is unavailable. */
  }
}
function reset() {
  name.value = selected.value?.name ?? "";
  source.value = selected.value ? JSON.stringify(selected.value.schema, null, 2) : "";
  initialName.value = name.value;
  initialSource.value = source.value;
  baseContext.value = context.value;
  format.value = "json";
  error.value = "";
  saved.value = false;
  clearDraft();
}

// Each model has an independent device draft, including incomplete source text.
function initialize() {
  name.value = selected.value?.name ?? "";
  source.value = selected.value ? JSON.stringify(selected.value.schema, null, 2) : "";
  initialName.value = name.value;
  initialSource.value = source.value;
  baseContext.value = context.value;
  try {
    const value: unknown = JSON.parse(localStorage.getItem(draftKey.value) ?? "null");
    if (isRecord(value) && typeof value.name === "string" && typeof value.source === "string" && typeof value.baseContext === "string") {
      name.value = value.name;
      source.value = value.source;
      baseContext.value = value.baseContext;
      format.value = value.format === "yaml" ? "yaml" : "json";
      editor.value = value.editor === "source" ? "source" : "fields";
    }
  } catch {
    /* A malformed device draft cannot replace the server contract. */
  }
}
initialize();
watch(
  () => props.content,
  () => {
    if (!busy.value && !dirty.value) reset();
  },
);
watch(
  [name, source, editor, format, baseContext],
  () => {
    saved.value = false;
    if (!selected.value || !dirty.value) {
      clearDraft();
      return;
    }
    try {
      localStorage.setItem(draftKey.value, JSON.stringify({ name: name.value, source: source.value, baseContext: baseContext.value, editor: editor.value, format: format.value }));
      storageFailed.value = false;
    } catch {
      storageFailed.value = true;
    }
  },
  { flush: "sync" },
);
watch(createOpen, (value) => {
  if (value) {
    createName.value = "";
    createDescription.value = "";
    error.value = "";
  }
});

function updateSchema(value: unknown) {
  source.value = JSON.stringify(value, null, 2);
  format.value = "json";
}
function showError(cause: unknown) {
  error.value = cause instanceof Error ? cause.message : t("specification.openapiModels.saveFailed");
}
function serialize(next: Record<string, unknown>) {
  return formatOpenApiContent(JSON.stringify(next), props.content.trimStart().startsWith("{") ? "json" : "yaml");
}

async function save() {
  if (!document.value || !selected.value || !props.editable || busy.value || conflict.value || validation.value || schemaInvalid.value || !dirty.value) return;
  const key = draftKey.value;
  const baseContent = props.content;
  let persistedDraft: string | null = null;
  try {
    persistedDraft = localStorage.getItem(key);
  } catch {
    /* Keep the in-memory draft. */
  }
  busy.value = true;
  error.value = "";
  try {
    const next = updateOpenApiModel(document.value, selected.value.reference, name.value, schemaValue.value);
    const valid = await validateOpenApiSchemaInWorker({ document: next, schema: schemaValue.value, body: undefined });
    if (valid.failure) {
      error.value = t("specification.openapiModels.invalidSchema");
      return;
    }
    if (props.content !== baseContent) {
      error.value = t("specification.openapiDesignConcurrentChange");
      return;
    }
    const parts = openApiPointerParts(selected.value.reference)!;
    const reference = openApiPointer([...parts.slice(0, -1), name.value.trim()]);
    if (await props.saveDocument(serialize(next), { kind: "model", previousReference: selected.value.reference, reference, title: name.value.trim() })) {
      // A newer view may already be editing this model while this save finishes.
      try {
        if (localStorage.getItem(key) === persistedDraft) clearDraft(key);
      } catch {
        /* Keep newer drafts intact. */
      }
      if (!disposed) {
        await nextTick();
        reset();
        saved.value = true;
      }
    } else error.value = t("specification.openapiModels.saveFailed");
  } catch (cause) {
    showError(cause);
  } finally {
    busy.value = false;
  }
}

async function create() {
  if (!document.value || !props.editable || busy.value || !createName.value.trim() || createValidation.value) return;
  busy.value = true;
  error.value = "";
  try {
    const next = updateOpenApiModel(document.value, undefined, createName.value, { type: "object", description: createDescription.value || undefined, properties: {} });
    if (await props.saveDocument(serialize(next), { kind: "model", reference: openApiPointer(["components", "schemas", createName.value.trim()]), title: createName.value.trim() })) createOpen.value = false;
    else error.value = t("specification.openapiModels.saveFailed");
  } catch (cause) {
    showError(cause);
  } finally {
    busy.value = false;
  }
}

async function remove() {
  if (!document.value || !selected.value || !props.editable || busy.value || blockers.value.length || dirty.value) return;
  busy.value = true;
  error.value = "";
  try {
    const next = deleteOpenApiModel(document.value, selected.value.reference);
    if (await props.saveDocument(serialize(next), { kind: "model", previousReference: selected.value.reference })) {
      clearDraft();
      deleteOpen.value = false;
    } else error.value = t("specification.openapiModels.saveFailed");
  } catch (cause) {
    showError(cause);
  } finally {
    busy.value = false;
  }
}

function downloadDraft() {
  const url = URL.createObjectURL(new Blob([source.value], { type: "text/plain;charset=utf-8" }));
  const link = globalThis.document.createElement("a");
  link.href = url;
  link.download = `${name.value.replace(/[^A-Za-z0-9._-]/g, "_") || "model"}.draft.${format.value}`;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
</script>

<template>
  <section class="openapi-model-panel min-h-0 min-w-0 flex-1 overflow-y-auto" :aria-label="t('specification.openapiModels.directory')">
    <template v-if="!props.modelRef">
      <header class="mb-5 flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <span class="eyebrow">{{ props.specificationTitle }}</span>
          <h2 class="flex items-center gap-2 text-xl font-semibold">
            <Layers3 class="size-5 text-primary" aria-hidden="true" />{{ t("specification.openapiModels.directory") }}<span class="rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground">{{ models.length }}</span>
          </h2>
          <p class="mt-2 max-w-2xl text-sm text-muted-foreground">{{ t("specification.openapiModels.description") }}</p>
        </div>
        <Button size="sm" :disabled="!props.editable || busy" @click="createOpen = true"><Plus class="size-3.5" aria-hidden="true" />{{ t("specification.openapiModels.create") }}</Button>
      </header>
      <div class="relative mb-5 max-w-lg">
        <Label for="openapi-model-search" class="sr-only">{{ t("specification.openapiModels.search") }}</Label
        ><Search class="pointer-events-none absolute top-2.5 left-3 size-4 text-muted-foreground" aria-hidden="true" /><Input id="openapi-model-search" v-model="query" name="openapi-model-search" autocomplete="off" class="pl-9" :placeholder="t('specification.openapiModels.search')" />
      </div>
      <div v-if="filtered.length" class="grid min-w-0 gap-3 lg:grid-cols-2">
        <button
          v-for="model in filtered"
          :key="model.reference"
          type="button"
          class="group flex min-w-0 items-start gap-3 rounded-lg border border-border bg-card p-4 text-left transition-colors hover:border-primary/50 hover:bg-muted/30 focus-visible:outline-2 focus-visible:outline-primary"
          @click="props.navigateModel(model.reference)"
        >
          <Layers3 class="mt-0.5 size-4 shrink-0 text-primary" aria-hidden="true" /><span class="min-w-0 flex-1"
            ><strong class="block truncate font-mono text-sm">{{ model.name }}</strong
            ><span class="mt-1 block line-clamp-2 text-xs leading-5 text-muted-foreground">{{ model.description || model.reference }}</span></span
          ><ArrowUpRight class="size-3.5 shrink-0 text-muted-foreground group-hover:text-primary" aria-hidden="true" />
        </button>
      </div>
      <div v-else class="rounded-lg border border-dashed border-border px-5 py-12 text-center text-sm text-muted-foreground">{{ query ? t("specification.openapiModels.noResults") : t("specification.openapiModels.empty") }}</div>
    </template>
    <template v-else-if="selected">
      <header class="mb-4 space-y-3">
        <Button size="xs" variant="ghost" @click="props.navigateModel()"><ArrowLeft class="size-3" aria-hidden="true" />{{ t("specification.openapiModels.directory") }}</Button>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <span class="eyebrow">{{ props.specificationTitle }}</span>
            <h2 class="flex items-center gap-2 text-xl font-semibold">
              <Layers3 class="size-5 shrink-0 text-primary" aria-hidden="true" /><span class="break-all">{{ selected.name }}</span>
            </h2>
            <p class="mt-1 break-all font-mono text-xs text-muted-foreground">{{ selected.reference }}</p>
          </div>
          <Button v-if="props.editable" size="sm" variant="destructive" :disabled="busy || dirty" @click="deleteOpen = true"><Trash2 class="size-3.5" aria-hidden="true" />{{ t("specification.openapiModels.delete") }}</Button>
        </div>
      </header>
      <TabsRoot v-model="mode" class="min-w-0" :unmount-on-hide="true">
        <TabsList class="mb-5 flex gap-5 border-b border-border" :aria-label="t('specification.openapiModels.views')"
          ><TabsTrigger value="preview" class="model-view-tab"><FileCode2 class="size-3.5" aria-hidden="true" />{{ t("specification.openapiDocumentationTab") }}</TabsTrigger
          ><TabsTrigger value="design" class="model-view-tab"><Braces class="size-3.5" aria-hidden="true" />{{ t("specification.openapiDesignTab") }}<span v-if="dirty" class="size-1.5 rounded-full bg-primary" :aria-label="t('specification.openapiDesignDraftLocal')" /></TabsTrigger
        ></TabsList>
        <TabsContent value="preview" class="min-w-0 focus-visible:outline-2 focus-visible:outline-primary">
          <div class="grid min-w-0 gap-5 xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div class="min-w-0 rounded-lg border border-border bg-card p-4">
              <OpenApiSchemaPreview :schema="{ $ref: selected.reference }" :document="document" mode="full" :navigate-model="props.navigateModel" />
              <details class="mt-5 border-t border-border pt-3">
                <summary class="cursor-pointer text-xs text-muted-foreground">{{ t("specification.openapiModels.source") }}</summary>
                <pre class="mt-3 max-h-96 overflow-auto rounded bg-muted/30 p-3 text-xs" tabindex="0">{{ JSON.stringify(selected.schema, null, 2) }}</pre>
              </details>
            </div>
            <aside class="min-w-0 space-y-3 rounded-lg border border-border bg-card p-4">
              <template v-if="dependencies.length"
                ><h3 class="text-sm font-medium">{{ t("specification.openapiModels.dependencies") }}</h3>
                <button
                  v-for="dependency in dependencies"
                  :key="dependency.reference"
                  type="button"
                  class="flex w-full items-center gap-2 rounded border border-border p-2 text-left text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-primary"
                  @click="props.navigateModel(dependency.reference)"
                >
                  <Layers3 class="size-3 shrink-0 text-primary" aria-hidden="true" /><span class="min-w-0 break-all font-mono">{{ dependency.name }}</span
                  ><ArrowUpRight class="ml-auto size-3 shrink-0" aria-hidden="true" />
                </button>
                <hr class="border-border"
              /></template>
              <h3 class="text-sm font-medium">{{ t("specification.openapiModels.usages", { count: usages.length }) }}</h3>
              <p class="text-xs leading-5 text-muted-foreground">{{ t("specification.openapiModels.usageHint") }}</p>
              <p v-if="!usages.length" class="text-xs text-muted-foreground">{{ t("specification.openapiModels.unused") }}</p>
              <button
                v-for="usage in usages"
                :key="`${usage.kind}:${usage.key}`"
                type="button"
                class="flex w-full min-w-0 items-start gap-2 rounded border border-border p-2 text-left text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-primary"
                @click="usage.kind === 'model' ? props.navigateModel(usage.key) : props.navigateOperation(usage.key, usage.title)"
              >
                <Layers3 v-if="usage.kind === 'model'" class="mt-0.5 size-3 shrink-0 text-primary" aria-hidden="true" /><FileCode2 v-else class="mt-0.5 size-3 shrink-0 text-primary" aria-hidden="true" /><span class="min-w-0 break-all font-mono"
                  >{{ usage.title }}<small v-if="!usage.direct" class="mt-1 block font-sans text-muted-foreground">{{ t("specification.openapiModels.indirect") }}</small></span
                ><ArrowUpRight class="ml-auto size-3 shrink-0" aria-hidden="true" />
              </button>
            </aside>
          </div>
        </TabsContent>
        <TabsContent value="design" class="min-w-0 space-y-4 focus-visible:outline-2 focus-visible:outline-primary">
          <div class="sticky top-0 z-10 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-border bg-background/95 p-3 shadow-sm backdrop-blur-sm">
            <p role="status" class="text-xs text-muted-foreground">{{ busy ? t("common.saving") : saved ? t("specification.openapiModels.saved") : dirty ? t("specification.openapiDesignDraftLocal") : t("specification.openapiModels.designHint") }}</p>
            <div class="flex flex-wrap gap-2">
              <Button v-if="dirty" size="sm" variant="outline" :disabled="busy" @click="downloadDraft">{{ t("specification.openapiDesignDownloadDraft") }}</Button>
              <Button size="sm" variant="outline" :disabled="busy || !dirty" @click="reset">{{ t("specification.openapiModels.reset") }}</Button
              ><Button size="sm" :disabled="busy || !props.editable || !dirty || conflict || Boolean(validation) || schemaInvalid" @click="save"
                ><LoaderCircle v-if="busy" class="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /><Check v-else class="size-3.5" aria-hidden="true" />{{ t("common.save") }}</Button
              >
            </div>
          </div>
          <div v-if="conflict" role="alert" class="space-y-2 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
            <strong>{{ t("specification.openapiModels.conflict") }}</strong>
            <p class="text-xs text-muted-foreground">{{ t("specification.openapiDesignDraftConflictDescription") }}</p>
            <Button size="xs" variant="outline" @click="downloadDraft">{{ t("specification.openapiDesignDownloadDraft") }}</Button>
          </div>
          <p v-if="!props.editable" class="rounded-lg border border-border p-3 text-sm text-muted-foreground">{{ t("specification.managedReadOnly") }}</p>
          <p class="rounded-lg border border-primary/20 bg-primary/5 px-3 py-2 text-xs leading-5 text-muted-foreground">{{ t("specification.openapiModels.impact", { count: usages.length }) }}</p>
          <p v-if="storageFailed" role="alert" class="text-sm text-destructive">{{ t("specification.openapiModels.storageFailed") }}</p>
          <p v-if="error && !deleteOpen" role="alert" class="text-sm text-destructive">{{ error }}</p>
          <div class="max-w-lg space-y-2">
            <Label for="openapi-model-name">{{ t("specification.openapiModels.name") }}</Label
            ><Input id="openapi-model-name" v-model="name" name="openapi-model-name" class="font-mono" :disabled="!props.editable || busy || conflict" :aria-invalid="Boolean(validation)" autocomplete="off" :spellcheck="false" />
            <p v-if="validation" role="alert" class="text-xs text-destructive">{{ t(`specification.openapiModels.${validation}`) }}</p>
          </div>
          <div class="flex gap-2" role="group" :aria-label="t('specification.openapiModels.editor')">
            <Button size="xs" :variant="editor === 'fields' ? 'secondary' : 'outline'" :aria-pressed="editor === 'fields'" @click="editor = 'fields'">{{ t("specification.openapiFields") }}</Button
            ><Button size="xs" :variant="editor === 'source' ? 'secondary' : 'outline'" :aria-pressed="editor === 'source'" @click="editor = 'source'">JSON / YAML</Button>
          </div>
          <div v-if="editor === 'source' && props.editable" class="h-[30rem] min-w-0"><OpenApiCodeEditor v-model="source" v-model:format="format" :read-only="busy || conflict" schema :label="t('specification.openapiModels.source')" /></div>
          <OpenApiSchemaDesignEditor v-else-if="isRecord(schemaValue) && editor === 'fields'" :model-value="schemaValue" :label="selected.name" :document="document" :ancestor-references="[selected.reference]" :editable="props.editable && !busy && !conflict" @update:model-value="updateSchema" />
          <p v-else class="rounded border border-border p-3 text-xs text-muted-foreground">{{ t("specification.openapiModels.sourceHint") }}</p>
          <p v-if="schemaInvalid" role="alert" class="text-sm text-destructive">{{ t("specification.openapiModels.invalidSchema") }}</p>
        </TabsContent>
      </TabsRoot>
    </template>
    <div v-else class="rounded-lg border border-dashed border-border p-8 text-center">
      <h2 class="text-base font-medium">{{ t("specification.openapiModels.missing") }}</h2>
      <Button class="mt-4" variant="outline" @click="props.navigateModel()">{{ t("specification.openapiModels.directory") }}</Button>
    </div>

    <Dialog :open="createOpen" @update:open="(value) => !busy && (createOpen = value)"
      ><DialogContent
        ><DialogHeader
          ><DialogTitle>{{ t("specification.openapiModels.create") }}</DialogTitle></DialogHeader
        >
        <form id="openapi-model-create-form" class="space-y-4" :aria-busy="busy" @submit.prevent="create">
          <div class="space-y-2">
            <Label for="openapi-model-create-name">{{ t("specification.openapiModels.name") }}</Label
            ><Input id="openapi-model-create-name" v-model="createName" name="openapi-model-create-name" placeholder="UserProfile" :disabled="busy" :aria-invalid="Boolean(createValidation)" autocomplete="off" :spellcheck="false" required />
            <p v-if="createValidation" role="alert" class="text-xs text-destructive">{{ t(`specification.openapiModels.${createValidation}`) }}</p>
          </div>
          <div class="space-y-2">
            <Label for="openapi-model-create-description">{{ t("specification.openapiDescription") }}</Label
            ><Input id="openapi-model-create-description" v-model="createDescription" name="openapi-model-create-description" autocomplete="off" :disabled="busy" />
          </div>
          <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        </form>
        <DialogFooter
          ><Button variant="outline" :disabled="busy" @click="createOpen = false">{{ t("common.cancel") }}</Button
          ><Button type="submit" form="openapi-model-create-form" :disabled="busy || !props.editable || !createName.trim() || Boolean(createValidation)"
            ><LoaderCircle v-if="busy" class="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />{{ t("specification.openapiModels.create") }}</Button
          ></DialogFooter
        ></DialogContent
      ></Dialog
    >
    <Dialog :open="deleteOpen" @update:open="(value) => !busy && (deleteOpen = value)"
      ><DialogContent
        ><DialogHeader
          ><DialogTitle>{{ t("specification.openapiModels.delete") }}</DialogTitle></DialogHeader
        >
        <p class="text-sm">{{ blockers.length ? t("specification.openapiModels.inUse", { count: blockers.length }) : t("specification.openapiModels.confirmDelete", { name: selected?.name ?? "" }) }}</p>
        <ul v-if="blockers.length" class="max-h-48 space-y-2 overflow-auto rounded border border-border p-3 text-xs">
          <li v-for="blocker in blockers" :key="blocker" class="break-all font-mono text-muted-foreground">{{ blocker }}</li>
        </ul>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <DialogFooter
          ><Button variant="outline" :disabled="busy" @click="deleteOpen = false">{{ t("common.cancel") }}</Button
          ><Button variant="destructive" :disabled="busy || !props.editable || Boolean(blockers.length)" @click="remove">{{ t("specification.openapiModels.delete") }}</Button></DialogFooter
        ></DialogContent
      ></Dialog
    >
  </section>
</template>
