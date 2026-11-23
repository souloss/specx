<script setup lang="ts">
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from "reka-ui";
import { useAppearancePortal } from "./useAppearancePortal";
defineProps<{ side?: "top" | "bottom"; align?: "start" | "center" | "end"; contentClass?: string }>();
const open = defineModel<boolean>("open", { default: false });
const portal = useAppearancePortal();
</script>
<template>
  <PopoverRoot v-model:open="open"
    ><PopoverTrigger as-child><slot name="trigger" /></PopoverTrigger
    ><PopoverPortal :to="portal"
      ><PopoverContent :side="side ?? 'bottom'" :align="align ?? 'start'" :side-offset="6" :collision-padding="10" class="app-popover" :class="contentClass"><slot /></PopoverContent></PopoverPortal
  ></PopoverRoot>
</template>
<style>
.app-popover {
  z-index: 100;
  max-width: calc(100vw - 24px);
  max-height: var(--reka-popover-content-available-height);
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: 12px;
  color: var(--foreground);
  background: var(--popover);
  box-shadow: 0 6px 24px #0002;
}
</style>
