<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { FolderTree, GitBranch, Plus, Tag } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { createId, displayGroupName } from "@/lib/core";
import { translate } from "@/i18n";
import { useToast } from "@/composables/useToast";
import { useWorkspaceStore } from "@/stores/workspace";

const props = withDefaults(defineProps<{ parentId?: string | null }>(), { parentId: null });
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const { toast } = useToast();
const t = translate;

const name = ref("");
const description = ref("");
const groupId = ref("ungrouped");
const tagIds = ref<string[]>([]);
const repositoryUrl = ref("");
const error = ref("");
const busy = ref(false);

const groups = computed(() => workspace.catalog?.groups ?? []);
const tags = computed(() => workspace.catalog?.tags ?? []);
const valid = computed(() => Boolean(name.value.trim()) && !busy.value);

function reset() {
  name.value = "";
  description.value = "";
  groupId.value = props.parentId ?? "ungrouped";
  tagIds.value = [];
  repositoryUrl.value = "";
  error.value = "";
  busy.value = false;
}

function toggleTag(id: string) {
  tagIds.value = tagIds.value.includes(id) ? tagIds.value.filter((tagId) => tagId !== id) : [...tagIds.value, id];
}

watch([open, () => props.parentId], ([isOpen]) => {
  if (isOpen) reset();
});

async function createService() {
  if (!valid.value) {
    error.value = t("service.nameRequired");
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    await workspace.createService({
      id: createId("svc"),
      name: name.value.trim(),
      description: description.value.trim(),
      groupId: groupId.value === "ungrouped" ? null : groupId.value,
      tagIds: tagIds.value,
      repositoryUrl: repositoryUrl.value.trim() || undefined,
    });
    open.value = false;
    toast(t("service.createSuccess"), 3000);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.createFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogContent class="grid-rows-[auto_minmax(0,1fr)] sm:max-w-xl" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2"><Plus class="size-4 text-primary" />{{ t("service.createTitle") }}</DialogTitle>
      </DialogHeader>

      <form class="min-h-0 space-y-5 overflow-y-auto" @submit.prevent="createService">
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
          <div class="space-y-2">
            <Label for="service-group"><FolderTree class="size-3.5" />{{ t("service.group") }}</Label>
            <Select v-model="groupId" :disabled="busy">
              <SelectTrigger id="service-group" class="w-full"><SelectValue :placeholder="t('service.groupPlaceholder')" /></SelectTrigger>
              <SelectContent>
                <SelectItem value="ungrouped">{{ t("common.ungrouped") }}</SelectItem>
                <SelectItem v-for="group in groups" :key="group.id" :value="group.id">{{ displayGroupName(group.id, group.name) }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="space-y-2">
            <Label for="service-repository"><GitBranch class="size-3.5" />{{ t("service.repositoryOptional") }}</Label>
            <Input id="service-repository" v-model="repositoryUrl" name="repositoryUrl" type="url" inputmode="url" :placeholder="t('service.repositoryUrlPlaceholder')" autocomplete="url" :disabled="busy" />
          </div>
        </div>

        <div class="space-y-2">
          <Label><Tag class="size-3.5" />{{ t("service.tags") }}</Label>
          <div v-if="tags.length" class="flex flex-wrap gap-2" role="group" :aria-label="t('service.tags')">
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
              <i class="size-2 rounded-full" :style="{ backgroundColor: tag.color }" aria-hidden="true" />{{ tag.name }}
            </button>
          </div>
          <p v-else class="text-xs text-muted-foreground">{{ t("service.noTagsAvailable") }}</p>
        </div>

        <p v-if="repositoryUrl.trim()" class="rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-muted-foreground">{{ t("service.repositoryOptionalDescription") }}</p>
        <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>

        <DialogFooter class="mx-0 mb-0">
          <Button type="button" variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || !name.trim()">{{ busy ? t("common.creating") : t("service.create") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
