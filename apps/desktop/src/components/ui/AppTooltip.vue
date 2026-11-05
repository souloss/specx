<script setup lang="ts">
import LightTooltip from "@/components/ui/LightTooltip.vue";

defineProps<{ text: string }>();
</script>

<template>
  <LightTooltip :text="text" side="bottom" :side-offset="0" content-class="pointer-events-none">
    <slot />
  </LightTooltip>
</template>
