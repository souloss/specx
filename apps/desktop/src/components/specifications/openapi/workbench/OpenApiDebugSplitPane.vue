<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { translate as t } from "@/i18n";
const root = ref<HTMLElement>();
const defaultRequestPercent = 46;
const requestPercent = ref(defaultRequestPercent);
let stopDrag: (() => void) | undefined;
function setPercent(percent: number) {
  requestPercent.value = Math.max(20, Math.min(85, percent));
}
function drag(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.currentTarget as HTMLElement;
  const rect = root.value?.getBoundingClientRect();
  if (!rect?.height) return;
  target.setPointerCapture(event.pointerId);
  const move = (e: PointerEvent) => setPercent(((e.clientY - rect.top) / rect.height) * 100);
  const stop = () => {
    target.removeEventListener("pointermove", move);
    target.removeEventListener("lostpointercapture", stop);
    stopDrag = undefined;
  };
  stopDrag?.();
  stopDrag = stop;
  target.addEventListener("pointermove", move);
  target.addEventListener("lostpointercapture", stop);
}
function keydown(event: KeyboardEvent) {
  if (!["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  setPercent(event.key === "Home" ? 20 : event.key === "End" ? 85 : requestPercent.value + (event.key === "ArrowUp" ? -5 : 5));
}
onBeforeUnmount(() => stopDrag?.());
</script>

<template>
  <div ref="root" class="openapi-debug-split" :style="{ '--request-percent': `${requestPercent}%` }">
    <div class="openapi-debug-request-scroll"><slot name="request" /></div>
    <div
      class="openapi-debug-resizer"
      role="separator"
      tabindex="0"
      aria-orientation="horizontal"
      :aria-label="t('specification.openapiResizeResponse')"
      :aria-valuenow="Math.round(requestPercent)"
      :aria-valuemin="20"
      :aria-valuemax="85"
      @pointerdown.prevent="drag"
      @keydown="keydown"
      @dblclick="requestPercent = defaultRequestPercent"
    >
      <span />
    </div>
    <div class="openapi-debug-response-scroll"><slot name="response" /></div>
  </div>
</template>
