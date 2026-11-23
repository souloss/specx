<script setup lang="ts">
import { History, MoreHorizontal, Redo2, RotateCcw, Undo2, Upload } from "@lucide/vue";
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from "reka-ui";
import { Button } from "@/components/ui/button";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";
import { translate as t } from "@/i18n";
defineProps<{ canUndo: boolean; canRedo: boolean }>();
const emit = defineEmits<{ action: [value: "undo" | "redo" | "reset" | "history" | "curl"] }>();
const portal = useAppearancePortal();
const { forwardRef } = useFloatingLayerOrder();
</script>
<template>
  <div class="debug-utility-actions">
    <DropdownMenuRoot>
      <DropdownMenuTrigger as-child
        ><Button size="icon-sm" variant="ghost" :aria-label="t('specification.openapiWorkbenchTools')"><MoreHorizontal class="size-3.5" /></Button
      ></DropdownMenuTrigger>
      <DropdownMenuPortal :to="portal">
        <DropdownMenuContent :ref="forwardRef" align="end" :side-offset="4" class="cn-menu-translucent z-(--specx-floating-layer-z-index) min-w-44 rounded-md border border-border p-1 text-popover-foreground shadow-lg">
          <DropdownMenuItem :disabled="!canUndo" class="debug-utility-menu-item" @select="emit('action', 'undo')"><Undo2 />{{ t("specification.openapiUndoRequest") }}</DropdownMenuItem>
          <DropdownMenuItem :disabled="!canRedo" class="debug-utility-menu-item" @select="emit('action', 'redo')"><Redo2 />{{ t("specification.openapiRedoRequest") }}</DropdownMenuItem>
          <DropdownMenuItem class="debug-utility-menu-item" @select="emit('action', 'reset')"><RotateCcw />{{ t("specification.openapiResetRequest") }}</DropdownMenuItem>
          <DropdownMenuItem class="debug-utility-menu-item" @select="emit('action', 'history')"><History />{{ t("specification.openapiHistory") }}</DropdownMenuItem>
          <DropdownMenuItem class="debug-utility-menu-item" @select="emit('action', 'curl')"><Upload />{{ t("specification.openapiImportCurl") }}</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenuPortal>
    </DropdownMenuRoot>
  </div>
</template>
