<script setup lang="ts">
import LightTooltip from "@/components/ui/LightTooltip.vue";

defineProps<{ text: string }>();
</script>

<template>
  <LightTooltip :text="text" side="bottom" :side-offset="4" :delay="0" :close-delay="0" nowrap content-class="pointer-events-none">
    <slot />
  </LightTooltip>
</template>
