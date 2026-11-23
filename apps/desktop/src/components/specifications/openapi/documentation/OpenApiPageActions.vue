<script setup lang="ts">
import { computed, ref } from "vue";
import { Check, ChevronDown, Clipboard, Code, Download, Link } from "@lucide/vue";
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from "reka-ui";
import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogTitle, DialogDescription, DialogScrollContent } from "@/components/ui/dialog";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";
import { downloadText } from "@/lib/transfer/downloadText";
import { translate as t } from "@/i18n";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { useAppearancePortal } from "../../../ui/useAppearancePortal";
const appearancePortal = useAppearancePortal();
const props = withDefaults(defineProps<{ markdown: string; definition?: string; allowExport?: boolean }>(), { allowExport: true });
const { copy, copied, failed } = useClipboardFeedback();
const preview = ref(false);
const menuTrigger = ref<InstanceType<typeof Button>>();
function returnFocus(event: Event) {
  event.preventDefault();
  const button = menuTrigger.value?.$el;
  if (button instanceof HTMLElement) button.focus();
}
const { forwardRef } = useFloatingLayerOrder();
const actions = computed(() => [
  { id: "markdown", label: t("openapiPublication.copyMarkdown"), icon: Clipboard },
  { id: "link", label: t("openapiPublication.copyPage"), icon: Link },
  { id: "view", label: t("openapiPublication.viewMarkdown"), icon: Code },
  ...(props.definition && props.allowExport ? [{ id: "definition", label: t("openapiPublication.copyDefinition"), icon: Code }] : []),
  ...(props.allowExport ? [{ id: "download", label: t("openapiPublication.downloadMarkdown"), icon: Download }] : []),
]);
function select(id: string) {
  if (id === "view") preview.value = true;
  else if (id === "download") downloadText(props.markdown, "api-document.md", "text/markdown;charset=utf-8");
  else void copy(id === "link" ? location.href : id === "definition" ? (props.definition ?? "") : props.markdown);
}
</script>
<template>
  <div class="space-y-1">
    <div class="inline-flex rounded-md">
      <Button variant="outline" size="sm" class="rounded-r-none" @click="copy(props.markdown)"
        ><Check v-if="copied" class="size-3.5" aria-hidden="true" /><Clipboard v-else class="size-3.5" aria-hidden="true" />{{ t(copied ? "specification.openapiCopied" : "openapiPublication.copyMarkdown") }}</Button
      >
      <DropdownMenuRoot>
        <DropdownMenuTrigger as-child
          ><Button ref="menuTrigger" variant="outline" size="sm" class="rounded-l-none border-l-0 px-2" :aria-label="t('openapiPublication.pageActions')"><ChevronDown class="size-3.5" aria-hidden="true" /></Button
        ></DropdownMenuTrigger>
        <DropdownMenuPortal :to="appearancePortal">
          <DropdownMenuContent
            :ref="forwardRef"
            align="end"
            :side-offset="4"
            class="cn-menu-translucent z-(--specx-floating-layer-z-index) min-w-48 max-w-[calc(100vw-1rem)] rounded-md border border-border p-1 text-popover-foreground shadow-lg"
            @close-auto-focus="(event) => preview && event.preventDefault()"
          >
            <DropdownMenuItem v-for="action in actions" :key="action.id" class="flex cursor-default items-center gap-2 rounded px-2 py-2 text-xs outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground" @select="select(action.id)"
              ><component :is="action.icon" class="size-3.5" aria-hidden="true" />{{ action.label }}</DropdownMenuItem
            >
          </DropdownMenuContent>
        </DropdownMenuPortal>
      </DropdownMenuRoot>
    </div>
    <p v-if="failed" role="status" class="text-xs text-destructive">{{ t("specification.openapiCopyFailed") }}</p>
    <Dialog :open="preview" @update:open="preview = $event">
      <DialogScrollContent class="gap-3 sm:max-w-3xl" @close-auto-focus="returnFocus">
        <DialogHeader
          ><DialogTitle>{{ t("openapiPublication.viewMarkdown") }}</DialogTitle
          ><DialogDescription>{{ t("openapiPublication.markdownHelp") }}</DialogDescription></DialogHeader
        >
        <OpenApiCodeSample :text="markdown" />
        <Button variant="outline" size="sm" @click="copy(props.markdown)">{{ t(copied ? "specification.openapiCopied" : "openapiPublication.copyMarkdown") }}</Button>
      </DialogScrollContent>
    </Dialog>
  </div>
</template>
