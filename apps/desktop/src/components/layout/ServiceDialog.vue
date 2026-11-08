<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { GitBranch, Pencil, Plus, Tag } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import TagBadge from "@/components/TagBadge.vue";
import { createId } from "@/lib/core";
import { translate } from "@/i18n";
import { useToast } from "@/composables/useToast";
import { useWorkspaceStore } from "@/stores/workspace";
import type { Service } from "@/types/domain";

const props = withDefaults(defineProps<{ parentId?: string | null; service?: Service | null }>(), { parentId: null, service: null });
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const { toast } = useToast();
const t = translate;

const name = ref("");
const description = ref("");
const tagIds = ref<string[]>([]);
const repositoryUrl = ref("");
const newTagName = ref("");
const newTagColor = ref("#64748b");
const tagsLoading = ref(false);
const error = ref("");
const busy = ref(false);

const tags = computed(() => workspace.catalog?.tags ?? []);
const editing = computed(() => Boolean(props.service));
const valid = computed(() => Boolean(name.value.trim()) && !busy.value);

function reset() {
  name.value = props.service?.name ?? "";
  description.value = props.service?.description ?? "";
  tagIds.value = props.service?.tagIds ? [...props.service.tagIds] : [];
  repositoryUrl.value = "";
  newTagName.value = "";
  newTagColor.value = "#64748b";
  error.value = "";
  busy.value = false;
}

function toggleTag(id: string) {
  tagIds.value = tagIds.value.includes(id) ? tagIds.value.filter((tagId) => tagId !== id) : [...tagIds.value, id];
}

async function addTag() {
  const name = newTagName.value.trim();
  if (!name || tagsLoading.value || tags.value.some((tag) => tag.name.toLocaleLowerCase() === name.toLocaleLowerCase())) return;
  tagsLoading.value = true;
  error.value = "";
  try {
    const created = await workspace.createTag({ id: createId("tag"), name, color: newTagColor.value });
    tagIds.value = [...tagIds.value, created.id];
    newTagName.value = "";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.tagCreateFailed");
  } finally {
    tagsLoading.value = false;
  }
}

watch([open, () => props.parentId, () => props.service?.id], ([isOpen]) => {
  if (!isOpen) return;
  reset();
  tagsLoading.value = true;
  void workspace.ensureDefaultTags().finally(() => {
    tagsLoading.value = false;
  });
});

async function saveService() {
  if (!valid.value) {
    error.value = t("service.nameRequired");
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    if (props.service) {
      await workspace.updateService({ ...props.service, name: name.value.trim(), description: description.value.trim(), tagIds: [...tagIds.value] });
    } else {
      await workspace.createService({
        id: createId("svc"),
        name: name.value.trim(),
        description: description.value.trim(),
        groupId: props.parentId ?? null,
        tagIds: tagIds.value,
        repositoryUrl: repositoryUrl.value.trim() || undefined,
      });
    }
    open.value = false;
    toast(t(editing.value ? "service.updateSuccess" : "service.createSuccess"), 3000);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t(editing.value ? "service.updateFailed" : "service.createFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogContent class="grid-rows-[auto_minmax(0,1fr)] sm:max-w-xl" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2"><Pencil v-if="editing" class="size-4 text-primary" /><Plus v-else class="size-4 text-primary" />{{ t(editing ? "service.editTitle" : "service.createTitle") }}</DialogTitle>
      </DialogHeader>

      <form class="min-h-0 space-y-5 overflow-y-auto" @submit.prevent="saveService">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2 sm:col-span-2">
            <Label for="service-name">{{ t("service.name") }}</Label>
            <Input id="service-name" v-model="name" name="name" :placeholder="t('service.namePlaceholder')" autocomplete="off" autofocus :disabled="busy" required />
          </div>
          <div class="space-y-2 sm:col-span-2">
            <Label for="service-description">{{ t("service.description") }}</Label>
            <textarea
              id="service-description"
              v-model="description"
              name="description"
              :placeholder="t('service.descriptionPlaceholder')"
              autocomplete="off"
              :disabled="busy"
              rows="3"
              class="min-h-20 w-full resize-y rounded-md border border-input bg-transparent px-2.5 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
            />
          </div>
          <div v-if="!editing" class="space-y-2">
            <Label for="service-repository"><GitBranch class="size-3.5" />{{ t("service.repositoryOptional") }}</Label>
            <Input id="service-repository" v-model="repositoryUrl" name="repositoryUrl" type="url" inputmode="url" :placeholder="t('service.repositoryUrlPlaceholder')" autocomplete="url" :disabled="busy" />
          </div>
        </div>

        <div class="space-y-2">
          <Label><Tag class="size-3.5" />{{ t("service.tags") }}</Label>
          <div v-if="tagsLoading" class="text-xs text-muted-foreground">{{ t("common.loading") }}</div>
          <div v-else-if="tags.length" class="flex flex-wrap gap-2" role="group" :aria-label="t('service.tags')">
            <button
              v-for="tag in tags"
              :key="tag.id"
              type="button"
              class="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs transition-colors focus-visible:ring-3 focus-visible:ring-ring/50"
              :class="tagIds.includes(tag.id) ? 'border-primary bg-primary/10 text-foreground' : 'border-border text-muted-foreground hover:bg-accent hover:text-foreground'"
              :aria-pressed="tagIds.includes(tag.id)"
              :disabled="busy"
              @click="toggleTag(tag.id)"
            >
              <TagBadge :tag="tag" compact />
            </button>
          </div>
          <div class="flex items-center gap-2">
            <Input v-model="newTagName" name="newTagName" class="h-8 min-w-0 flex-1" :placeholder="t('service.newTagPlaceholder')" :aria-label="t('service.newTagPlaceholder')" autocomplete="off" :disabled="busy || tagsLoading" @keydown.enter.prevent="addTag" />
            <input v-model="newTagColor" name="newTagColor" type="color" class="h-8 w-10 cursor-pointer rounded-md border border-input bg-transparent p-1" :aria-label="t('service.tagColor')" :disabled="busy || tagsLoading" />
            <Button type="button" variant="outline" size="sm" :disabled="busy || tagsLoading || !newTagName.trim()" @click="addTag"><Plus class="size-3.5" />{{ t("service.addTag") }}</Button>
          </div>
        </div>

        <p v-if="!editing && repositoryUrl.trim()" class="rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">{{ t("service.repositoryOptionalDescription") }}</p>
        <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>

        <DialogFooter class="mx-0 mb-0">
          <Button type="button" variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || !name.trim()">{{ busy ? t("common.saving") : t(editing ? "common.save" : "service.create") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
