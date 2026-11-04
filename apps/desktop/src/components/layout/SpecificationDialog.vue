<script setup lang="ts">
import { ref, watch } from "vue";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useWorkspaceStore } from "@/stores/workspace";
import { specxApi } from "@/lib/backend/client";
import { createId } from "@/lib/core";
import { translate } from "@/i18n";
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const title = ref("");
const content = ref(t("specification.defaultContent"));
const error = ref("");
const busy = ref(false);
const serviceId = ref("");
watch(open, (value) => {
  if (value) {
    title.value = "";
    content.value = t("specification.defaultContent");
    error.value = "";
    serviceId.value = workspace.activeServiceId ?? "";
  }
});
async function save() {
  if (!workspace.catalog || !serviceId.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    const id = createId("spec");
    await specxApi.createSpecification(workspace.catalog.workspace.id, { id, serviceId: serviceId.value, title: title.value.trim(), content: content.value });
    await workspace.refresh();
    workspace.openSpecification(id);
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.createFailed");
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <Dialog
    :open="open"
    @update:open="
      (value) => {
        if (!busy) open = value;
      }
    "
    ><DialogContent class="sm:max-w-2xl" :show-close-button="!busy" @escape-key-down="busy && $event.preventDefault()" @pointer-down-outside="busy && $event.preventDefault()"
      ><DialogHeader
        ><DialogTitle>{{ t("specification.createTitle") }}</DialogTitle></DialogHeader
      >
      <form class="flex min-h-0 flex-col gap-4" @submit.prevent="save">
        <label class="settings-field">{{ t("specification.title") }}<input v-model="title" class="settings-input" required /></label
        ><label class="settings-field">{{ t("specification.content") }}<textarea v-model="content" class="settings-input min-h-56 resize-y font-mono" rows="12" required /></label>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <DialogFooter
          ><Button type="button" variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button
          ><Button type="submit" :disabled="busy || !title.trim() || !content.trim()">{{ busy ? t("common.creating") : t("specification.create") }}</Button></DialogFooter
        >
      </form></DialogContent
    ></Dialog
  >
</template>
