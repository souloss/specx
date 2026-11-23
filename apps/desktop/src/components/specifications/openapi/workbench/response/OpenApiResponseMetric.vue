<script setup lang="ts">
import { ref, useId } from "vue";
import { TooltipContent, TooltipPortal, TooltipProvider, TooltipRoot, TooltipTrigger } from "reka-ui";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";

defineProps<{ label: string; kind: string }>();
const open = ref(false);
const id = useId();
const appearancePortal = useAppearancePortal();
const { forwardRef } = useFloatingLayerOrder();
</script>

<template>
  <TooltipProvider :delay-duration="300" :skip-delay-duration="0">
    <TooltipRoot v-model:open="open" :disable-hoverable-content="false" :disable-closing-trigger="true">
      <TooltipTrigger as-child>
        <button type="button" class="response-metric-trigger" :data-metric="kind" :aria-label="label" :aria-expanded="open" :aria-details="open ? id : undefined" @click="open = true">
          <slot />
        </button>
      </TooltipTrigger>
      <TooltipPortal :to="appearancePortal">
        <TooltipContent :id="id" :ref="forwardRef" class="response-metric-card" :class="`response-metric-card-${kind}`" side="bottom" align="end" :side-offset="8" :collision-padding="12">
          <slot name="details" />
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>
