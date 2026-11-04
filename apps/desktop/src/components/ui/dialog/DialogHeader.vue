<!-- Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. -->
<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/common/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <div data-slot="dialog-header" :class="cn('gap-2 flex flex-col', props.class)">
    <slot />
  </div>
</template>
