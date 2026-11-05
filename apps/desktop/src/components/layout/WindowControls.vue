<script setup lang="ts">
import { Copy, Expand, Minimize2, Minus, Square, X } from "@lucide/vue";
import { translate } from "@/i18n";

defineProps<{ isMaximized: boolean; isFullscreen: boolean }>();
const emit = defineEmits<{ minimize: []; "toggle-maximize": []; "toggle-fullscreen": []; close: [] }>();
const t = translate;
</script>

<template>
  <div class="flex h-10 shrink-0 items-stretch" data-window-controls>
    <button type="button" class="window-control" :aria-label="t('toolbar.minimize')" @click="emit('minimize')"><Minus class="h-4 w-4" /></button>
    <button type="button" class="window-control" :aria-label="isMaximized ? t('toolbar.restore') : t('toolbar.maximize')" @click="emit('toggle-maximize')">
      <Copy v-if="isMaximized" class="h-3.5 w-3.5" />
      <Square v-else class="h-3.5 w-3.5" />
    </button>
    <button type="button" class="window-control" :aria-label="isFullscreen ? t('toolbar.exitFullscreen') : t('toolbar.fullscreen')" @click="emit('toggle-fullscreen')">
      <Minimize2 v-if="isFullscreen" class="h-3.5 w-3.5" />
      <Expand v-else class="h-3.5 w-3.5" />
    </button>
    <button type="button" class="window-control window-control-close" :aria-label="t('toolbar.close')" @click="emit('close')"><X class="h-4 w-4" /></button>
  </div>
</template>

<style scoped>
.window-control {
  display: inline-flex;
  width: 2.75rem;
  align-items: center;
  justify-content: center;
  color: var(--foreground);
  transition: background-color 120ms ease;
}
.window-control:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}
.window-control:active {
  background: color-mix(in srgb, var(--foreground) 18%, transparent);
}
.window-control-close:hover {
  background: #dc2626;
  color: white;
}
</style>
