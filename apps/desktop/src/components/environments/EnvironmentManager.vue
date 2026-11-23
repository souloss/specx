<script setup lang="ts">
import { Copy, Download, Ellipsis, Plus, Trash2 } from "@lucide/vue";
import { DropdownMenuContent, DropdownMenuItem, DropdownMenuPortal, DropdownMenuRoot, DropdownMenuTrigger } from "reka-ui";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { translate as t } from "@/i18n";

defineProps<{
  items: { id: string; name: string }[];
  selectedId: string | null;
  busy: boolean;
  contentClass?: string;
}>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ select: [id: string]; add: []; duplicate: [id: string]; export: [id: string]; delete: [id: string] }>();
const portal = useAppearancePortal();
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="environment-manager-dialog p-0 gap-0" :class="contentClass" @interact-outside.prevent @escape-key-down="busy && $event.preventDefault()">
      <DialogHeader class="environment-manager-header">
        <DialogTitle>{{ t("environmentContext.settings") }}</DialogTitle>
      </DialogHeader>
      <div class="environment-manager-layout">
        <nav class="environment-manager-sidebar" :aria-label="t('requestAgent.environment')">
          <p class="environment-manager-heading">{{ t("requestAgent.environment") }}</p>
          <div v-for="item in items" :key="item.id" class="environment-manager-item" :class="{ active: selectedId === item.id }">
            <button type="button" class="environment-manager-option" :aria-current="selectedId === item.id ? 'true' : undefined" :disabled="busy" @click="emit('select', item.id)">
              <span class="environment-avatar" aria-hidden="true">{{ item.name.trim().slice(0, 1).toUpperCase() }}</span>
              <span class="truncate" :title="item.name">{{ item.name }}</span>
            </button>
            <DropdownMenuRoot>
              <DropdownMenuTrigger as-child>
                <Button variant="ghost" size="icon-sm" class="environment-manager-more" :aria-label="t('environmentContext.actions', { name: item.name })" :disabled="busy"><Ellipsis class="size-4" /></Button>
              </DropdownMenuTrigger>
              <DropdownMenuPortal :to="portal">
                <DropdownMenuContent :side-offset="5" :collision-padding="12" align="end" class="environment-actions-menu">
                  <DropdownMenuItem class="environment-action" @select="emit('duplicate', item.id)"><Copy class="size-4" />{{ t("environmentContext.duplicate") }}</DropdownMenuItem>
                  <DropdownMenuItem class="environment-action" @select="emit('export', item.id)"><Download class="size-4" />{{ t("environmentContext.export") }}</DropdownMenuItem>
                  <DropdownMenuItem class="environment-action text-destructive" @select="emit('delete', item.id)"><Trash2 class="size-4" />{{ t("common.delete") }}</DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenuPortal>
            </DropdownMenuRoot>
          </div>
          <Button type="button" size="sm" variant="ghost" class="environment-manager-add" :disabled="busy" @click="emit('add')"><Plus class="size-4" />{{ t("environmentContext.add") }}</Button>
        </nav>
        <div class="environment-manager-main"><slot /></div>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style>
.environment-manager-dialog[data-slot="dialog-content"] {
  width: 100%;
  max-width: none;
  height: calc(var(--specx-viewport-height) - 2rem);
  grid-template-rows: auto minmax(0, 1fr);
}
.environment-manager-header {
  padding: 14px 52px 14px 20px;
  border-block-end: 1px solid var(--border);
}
.environment-manager-header h2 {
  font-size: 14px;
  font-weight: 500;
}
.environment-manager-layout {
  display: grid;
  grid-template-columns: clamp(11rem, 18%, 17.5rem) minmax(0, 1fr);
  min-height: 0;
}
.environment-manager-sidebar {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 16px;
  border-inline-end: 1px solid var(--border);
  overflow: auto;
  overscroll-behavior: contain;
}
.environment-manager-heading {
  padding: 4px 6px;
  color: var(--muted-foreground);
  font-size: 12px;
}
.environment-manager-item {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  border-radius: 7px;
}
.environment-manager-item:hover,
.environment-manager-item.active,
.environment-manager-item:focus-within {
  background: var(--accent);
  color: var(--accent-foreground);
}
.environment-manager-option {
  display: flex;
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  text-align: start;
}
.environment-avatar {
  display: inline-grid;
  flex: none;
  width: 24px;
  height: 24px;
  place-items: center;
  border-radius: 5px;
  background: color-mix(in srgb, var(--primary) 10%, transparent);
  color: var(--primary);
  font-size: 12px;
}
.environment-manager-more {
  margin-inline-end: 3px;
}
@media (hover: hover) {
  .environment-manager-item:not(.active, :hover, :focus-within) .environment-manager-more:not([data-state="open"]) {
    opacity: 0;
    pointer-events: none;
  }
}
.environment-manager-add {
  align-self: flex-start;
  margin-block-start: 5px;
}
.environment-manager-main {
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
}
.environment-actions-menu {
  z-index: 100;
  min-width: 130px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--popover);
  color: var(--popover-foreground);
  box-shadow: 0 6px 20px #0002;
}
.environment-action {
  display: flex;
  align-items: center;
  gap: 8px;
  border-radius: 4px;
  padding: 7px 9px;
  font-size: 13px;
}
.environment-action[data-highlighted] {
  background: var(--accent);
  outline: none;
}
@media (max-width: 640px) {
  .environment-manager-layout {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
  }
  .environment-manager-sidebar {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 130px), 1fr));
    max-height: 24dvh;
    padding: 6px 10px;
    border-inline-end: 0;
    border-block-end: 1px solid var(--border);
  }
  .environment-manager-heading {
    grid-column: 1 / -1;
  }
  .environment-manager-add {
    justify-self: start;
    margin: 0;
  }
}
</style>
