<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Code2, GitBranch, Link2, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";

const props = defineProps<{ serviceId: string }>();
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const error = ref("");
const busy = ref(false);
const loaded = ref(false);
const repositoryUrl = ref("");
const confirmUnlink = ref(false);
const linkedRepository = computed(() => workspace.serviceRepositoryFor(props.serviceId));
const isLinked = computed(() => Boolean(linkedRepository.value));
let loadRevision = 0;

watch(
  () => [open.value, props.serviceId] as const,
  async ([value, serviceId]) => {
    const revision = ++loadRevision;
    if (!value) return;
    error.value = "";
    repositoryUrl.value = "";
    confirmUnlink.value = false;
    loaded.value = false;
    busy.value = true;
    try {
      await workspace.loadServiceRepository(serviceId);
    } catch (cause) {
      if (revision === loadRevision) error.value = cause instanceof Error ? cause.message : t("service.repositoryLoadFailed");
    } finally {
      if (revision === loadRevision) {
        loaded.value = true;
        busy.value = false;
      }
    }
  },
);

async function linkRepository() {
  const value = repositoryUrl.value.trim();
  if (busy.value || !value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.linkServiceRepository(props.serviceId, value);
    repositoryUrl.value = "";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.repositorySaveFailed");
  } finally {
    busy.value = false;
  }
}

async function unlinkRepository() {
  const repository = linkedRepository.value;
  if (!repository || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.unlinkServiceRepository(props.serviceId, repository.etag);
    confirmUnlink.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.repositorySaveFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogScrollContent class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 max-h-[calc(var(--specx-viewport-height)-2rem)] sm:max-w-xl">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle class="flex items-center gap-2 text-base"><Code2 class="size-4" />{{ t("service.codeBindings") }}</DialogTitle>
      </DialogHeader>

      <div class="min-h-0 space-y-5 overflow-y-auto overscroll-contain px-5 py-4">
        <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>

        <p v-if="!loaded" class="py-8 text-center text-sm text-muted-foreground">{{ t("service.repositoryLoading") }}</p>

        <section v-else-if="!isLinked" class="space-y-4">
          <div>
            <h3 class="text-sm font-medium">{{ t("service.repositoryNotLinked") }}</h3>
            <p class="mt-1 text-sm text-muted-foreground">{{ t("service.repositoryNotLinkedDescription") }}</p>
          </div>
          <div class="space-y-2">
            <Label for="repository-url">{{ t("service.repositoryUrl") }}</Label>
            <Input id="repository-url" v-model="repositoryUrl" type="url" inputmode="url" :placeholder="t('service.repositoryUrlPlaceholder')" :disabled="busy" @keydown.enter.prevent="linkRepository" />
          </div>
        </section>

        <template v-else-if="linkedRepository">
          <section class="space-y-3">
            <h3 class="text-sm font-medium">{{ t("service.repositoryDetails") }}</h3>
            <div class="min-w-0 border-l-2 border-primary pl-3">
              <div class="flex items-center gap-2 text-sm font-medium">
                <GitBranch class="size-4 shrink-0 text-primary" /><span class="truncate">{{ linkedRepository.repository.displayName }}</span
                ><span class="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">{{ linkedRepository.repository.provider }}</span>
              </div>
              <code class="mt-1 block truncate text-xs text-muted-foreground">{{ linkedRepository.repository.canonicalUrl }}</code>
            </div>
          </section>

          <p class="rounded-md border border-dashed border-border px-3 py-4 text-sm text-muted-foreground">{{ t("service.repositoryBindingDescription") }}</p>
        </template>
      </div>

      <DialogFooter class="mx-0 mb-0 border-t border-border bg-muted/30 px-5 py-3 sm:flex-row">
        <Button variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button>
        <Button v-if="loaded && !isLinked" :disabled="busy || !repositoryUrl.trim()" @click="linkRepository"><Link2 class="size-4" />{{ t("service.linkRepository") }}</Button>
        <Button v-if="loaded && isLinked" variant="destructive" :disabled="busy" @click="confirmUnlink = true"><Trash2 class="size-4" />{{ t("service.unlinkRepository") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>

  <Dialog :open="confirmUnlink" @update:open="(value) => !busy && (confirmUnlink = value)">
    <DialogScrollContent class="sm:max-w-md">
      <DialogHeader
        ><DialogTitle>{{ t("service.unlinkRepository") }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("service.confirmUnlinkRepository") }}</p>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <DialogFooter>
        <Button variant="outline" :disabled="busy" @click="confirmUnlink = false">{{ t("common.cancel") }}</Button>
        <Button variant="destructive" :disabled="busy" @click="unlinkRepository"><Trash2 class="size-4" />{{ t("service.unlinkRepository") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
