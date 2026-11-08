<script setup lang="ts">
import { computed } from "vue";
import type { Tag } from "@/types/domain";

const props = withDefaults(defineProps<{ tag: Tag; compact?: boolean }>(), { compact: false });

const knownLogos: Record<string, string> = {
  api: "API",
  javascript: "JS",
  kubernetes: "K8S",
  markdown: "MD",
  python: "PY",
  react: "R",
  rust: "RS",
  typescript: "TS",
  vue: "V",
};
const logo = computed(() => knownLogos[props.tag.name.trim().toLocaleLowerCase()] ?? props.tag.name.trim().slice(0, 2).toLocaleUpperCase());
</script>

<template>
  <span class="inline-flex min-w-0 items-center gap-1.5" :class="compact ? 'text-[11px]' : 'rounded-full border border-border bg-muted/40 px-2 py-1 text-xs'">
    <span class="inline-flex shrink-0 items-center justify-center rounded-full font-semibold leading-none text-white shadow-sm" :class="compact ? 'size-4 text-[7px]' : 'size-5 text-[8px]'" :style="{ backgroundColor: tag.color }" aria-hidden="true">{{ logo }}</span>
    <span class="truncate">{{ tag.name }}</span>
  </span>
</template>
