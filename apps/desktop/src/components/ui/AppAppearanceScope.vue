<script setup lang="ts">
import { provide, ref } from "vue";
import { appearancePortalKey } from "./useAppearancePortal";

defineProps<{ theme: "light" | "dark" }>();
const portal = ref<HTMLElement>();
provide(appearancePortalKey, portal);
</script>

<template>
  <div class="app-appearance-scope bg-background text-foreground" :class="{ dark: theme === 'dark' }" :data-appearance="theme" :style="{ colorScheme: theme }">
    <slot />
    <div ref="portal" data-appearance-portals />
  </div>
</template>

<style>
.app-appearance-scope {
  --bg: var(--background);
  --surface: var(--card);
  --surface-muted: var(--muted);
  --text: var(--foreground);
}
.app-appearance-scope[data-appearance] .cn-menu-translucent {
  background-color: var(--popover) !important;
  border-color: var(--border) !important;
}
</style>
