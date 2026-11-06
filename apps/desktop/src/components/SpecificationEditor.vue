<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Eye, FileEdit, Save, Split, X } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import MarkdownPreview from "./MarkdownPreview.vue";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Specification } from "@/types/domain";
import { translate } from "@/i18n";

const props = defineProps<{ specification: Specification }>();
const emit = defineEmits<{ close: []; saved: [] }>();
const workspace = useWorkspaceStore();
const t = translate;
const title = ref("");
const content = ref("");
const mode = ref<"edit" | "split" | "preview">("split");
const ready = ref(false);
const titleInput = ref<HTMLInputElement>();
const confirmClose = ref(false);

const draft = computed(() => workspace.specificationDraft(props.specification.id));
const dirty = computed(() => workspace.hasUnsavedSpecification(props.specification.id));
const status = computed(() => draft.value?.status ?? "saved");
const remote = computed(() => draft.value?.remote);
const canSave = computed(() => dirty.value && Boolean(title.value.trim()) && Boolean(content.value.trim()) && status.value !== "saving");
const wordCount = computed(() => content.value.trim().split(/\s+/).filter(Boolean).length);
const lineCount = computed(() => (content.value ? content.value.split("\n").length : 0));

function syncFromStore() {
  const saved = workspace.beginSpecificationEdit(props.specification.id);
  title.value = saved?.title ?? props.specification.title;
  content.value = saved?.content ?? props.specification.content;
  ready.value = true;
}

watch(
  () => [props.specification.id, props.specification.version, draft.value?.status, draft.value?.title, draft.value?.content] as const,
  () => {
    if (!ready.value || status.value === "saving") return;
    const saved = workspace.specificationDraft(props.specification.id);
    if (!saved) {
      title.value = props.specification.title;
      content.value = props.specification.content;
    } else if (saved.title !== title.value || saved.content !== content.value) {
      title.value = saved.title;
      content.value = saved.content;
    }
  },
);

watch(
  () => props.specification.id,
  () => {
    ready.value = false;
    syncFromStore();
    void nextTick(() => titleInput.value?.focus());
  },
  { immediate: true },
);

watch([title, content], ([nextTitle, nextContent]) => {
  if (ready.value) workspace.updateSpecificationDraft(props.specification.id, { title: nextTitle, content: nextContent });
});

async function save() {
  if (!canSave.value) return;
  const saved = await workspace.saveSpecification(props.specification.id);
  if (saved) emit("saved");
}

function useRemote() {
  workspace.discardSpecificationDraft(props.specification.id);
  title.value = props.specification.title;
  content.value = props.specification.content;
}

function requestClose() {
  if (dirty.value) confirmClose.value = true;
  else emit("close");
}

async function saveAndClose() {
  if (!(await workspace.saveSpecification(props.specification.id))) return;
  confirmClose.value = false;
  emit("close");
}

function discardAndClose() {
  workspace.discardSpecificationDraft(props.specification.id);
  confirmClose.value = false;
  emit("close");
}
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col gap-3">
    <header class="flex flex-wrap items-start justify-between gap-3 border-b border-border pb-3">
      <div class="min-w-0 flex-1">
        <label class="sr-only" :for="`specification-title-${specification.id}`">{{ t("specification.title") }}</label>
        <input :id="`specification-title-${specification.id}`" ref="titleInput" v-model="title" class="w-full border-0 bg-transparent px-0 text-xl font-semibold outline-none focus:ring-0" :placeholder="t('specification.title')" />
        <div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <span>{{ t("specification.currentVersion", { version: specification.version }) }}</span>
          <span aria-hidden="true">·</span>
          <span>{{ t("specification.lineCount", { count: lineCount }) }}</span>
          <span aria-hidden="true">·</span>
          <span>{{ t("specification.wordCount", { count: wordCount }) }}</span>
          <span v-if="dirty" class="text-amber-600 dark:text-amber-400">· {{ t("specification.unsavedChanges") }}</span>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <div class="flex items-center rounded-md border border-border p-0.5" role="group" :aria-label="t('specification.preview')">
          <button type="button" class="rounded p-1.5" :class="mode === 'edit' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent'" :aria-label="t('specification.editor')" :aria-pressed="mode === 'edit'" @click="mode = 'edit'"><FileEdit class="h-3.5 w-3.5" /></button>
          <button type="button" class="rounded p-1.5" :class="mode === 'split' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent'" :aria-label="t('specification.splitView')" :aria-pressed="mode === 'split'" @click="mode = 'split'"><Split class="h-3.5 w-3.5" /></button>
          <button type="button" class="rounded p-1.5" :class="mode === 'preview' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent'" :aria-label="t('specification.preview')" :aria-pressed="mode === 'preview'" @click="mode = 'preview'"><Eye class="h-3.5 w-3.5" /></button>
        </div>
        <Button size="sm" :disabled="!canSave" @click="save"><Save class="h-3.5 w-3.5" />{{ status === "saving" ? t("common.saving") : t("common.save") }}</Button>
        <Button variant="ghost" size="icon-sm" :aria-label="t('common.close')" @click="requestClose"><X class="h-4 w-4" /></Button>
      </div>
    </header>

    <div v-if="status === 'conflict'" class="flex flex-wrap items-center justify-between gap-3 rounded-md border border-amber-300 bg-amber-50 p-3 text-sm text-amber-950 dark:border-amber-700 dark:bg-amber-950/30 dark:text-amber-100" role="alert">
      <div>
        <strong class="block">{{ t("specification.saveConflict") }}</strong>
        <span class="text-xs opacity-80"
          >{{ t("specification.conflictDescription") }}<template v-if="remote"> {{ t("specification.remoteVersion", { version: remote.version }) }}</template></span
        >
      </div>
      <div class="flex items-center gap-2">
        <Button size="sm" variant="outline" @click="useRemote">{{ t("specification.useRemote") }}</Button>
        <Button size="sm" variant="secondary" @click="workspace.updateSpecificationDraft(specification.id, { title, content })">{{ t("specification.keepDraft") }}</Button>
      </div>
    </div>
    <div v-if="status === 'conflict' && remote" class="min-h-0 rounded-md border border-amber-300 bg-amber-50/40 p-3 dark:border-amber-700 dark:bg-amber-950/10">
      <div class="mb-2 text-xs font-medium text-muted-foreground">{{ t("specification.remoteVersion", { version: remote.version }) }}</div>
      <div class="max-h-48 overflow-y-auto rounded border border-border bg-background p-3"><MarkdownPreview :content="remote.content" /></div>
    </div>
    <p v-if="status === 'error'" role="alert" class="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">{{ draft?.error || t("specification.saveFailed") }}</p>

    <div class="grid min-h-0 flex-1 gap-3" :class="mode === 'split' ? 'lg:grid-cols-2' : 'grid-cols-1'">
      <textarea
        v-if="mode !== 'preview'"
        v-model="content"
        class="min-h-[360px] w-full resize-none rounded-md border border-border bg-muted/10 p-4 font-mono text-sm leading-relaxed outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
        :aria-label="t('specification.content')"
        spellcheck="false"
      />
      <div v-if="mode !== 'edit'" class="min-h-[360px] min-w-0 overflow-auto rounded-md border border-border bg-background p-4">
        <MarkdownPreview :content="content" />
      </div>
    </div>
    <footer class="flex items-center justify-between text-xs text-muted-foreground">
      <span v-if="status === 'saved'">{{ t("common.saved") }}</span>
      <span v-else-if="status === 'saving'">{{ t("common.saving") }}</span>
      <span v-else>{{ t("specification.unsavedChanges") }}</span>
      <span>{{ t("service.updatedAt", { date: specification.updatedAt }) }}</span>
    </footer>
  </section>
  <Dialog :open="confirmClose" @update:open="(value) => !value && (confirmClose = false)">
    <DialogContent>
      <DialogHeader
        ><DialogTitle>{{ t("specification.closeWithUnsavedTitle") }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("specification.closeWithUnsavedDescription") }}</p>
      <DialogFooter>
        <Button variant="outline" @click="confirmClose = false">{{ t("common.cancel") }}</Button>
        <Button variant="ghost" @click="discardAndClose">{{ t("specification.discardAndClose") }}</Button>
        <Button @click="saveAndClose"><Save class="h-3.5 w-3.5" />{{ t("specification.saveAndClose") }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
