<script setup lang="ts">
// Text hints never receive pointer events.
import { TooltipProvider, TooltipRoot, TooltipTrigger, TooltipPortal, TooltipContent, TooltipArrow } from "reka-ui";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";

defineProps<{ text: string }>();
const { forwardRef } = useFloatingLayerOrder();
</script>

<template>
  <TooltipProvider :delay-duration="300" :skip-delay-duration="0" :disable-hoverable-content="true">
    <TooltipRoot>
      <TooltipTrigger as-child><slot /></TooltipTrigger>
      <TooltipPortal>
        <TooltipContent :ref="forwardRef" data-slot="tooltip-content" data-specx-text-tooltip side="bottom" :side-offset="0" :collision-padding="8" class="pointer-events-none z-(--specx-floating-layer-z-index) max-w-xs rounded-md bg-foreground px-3 py-1.5 text-xs text-background-solid shadow-md">
          {{ text }}
          <TooltipArrow class="fill-foreground" />
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>

<style>
[data-reka-popper-content-wrapper]:has([data-specx-text-tooltip]) {
  pointer-events: none;
}
</style>
