<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
export type OpenApiOperationTab = { value: string; id: string; label: string; count?: number; status?: string };

const props = defineProps<{
  modelValue: string;
  tabs: OpenApiOperationTab[];
  ariaLabel: string;
}>();

const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const tablist = ref<HTMLElement>();

function buttonFor(value: string) {
  return [...(tablist.value?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? [])].find((button) => button.dataset.value === value);
}

function reveal(value: string) {
  const list = tablist.value;
  const button = buttonFor(value);
  if (!list || !button) return;
  const bounds = list.getBoundingClientRect();
  const tab = button.getBoundingClientRect();
  // Scroll only this tablist; revealing a tab must not move the document or split panes.
  if (tab.left < bounds.left) list.scrollLeft -= bounds.left - tab.left;
  else if (tab.right > bounds.right) list.scrollLeft += tab.right - bounds.right;
}

function select(value: string) {
  emit("update:modelValue", value);
  reveal(value);
}

watch(
  () => props.modelValue,
  async (value) => {
    await nextTick();
    reveal(value);
  },
);

function keydown(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) || !props.tabs.length) return;
  event.preventDefault();
  const currentIndex = Math.max(
    0,
    props.tabs.findIndex((tab) => tab.value === props.modelValue),
  );
  const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? props.tabs.length - 1 : (currentIndex + (event.key === "ArrowRight" ? 1 : -1) + props.tabs.length) % props.tabs.length;
  const next = props.tabs[nextIndex];
  if (!next) return;
  select(next.value);
  buttonFor(next.value)?.focus({ preventScroll: true });
}
</script>

<template>
  <div ref="tablist" class="openapi-operation-tabs" role="tablist" :aria-label="props.ariaLabel" @keydown="keydown">
    <button
      v-for="tab in props.tabs"
      :id="tab.id"
      :key="tab.value"
      type="button"
      role="tab"
      :data-value="tab.value"
      class="openapi-operation-tab"
      :class="{ active: props.modelValue === tab.value }"
      :aria-selected="props.modelValue === tab.value"
      :tabindex="props.modelValue === tab.value ? 0 : -1"
      @click="select(tab.value)"
    >
      <slot name="tab" :tab="tab">
        {{ tab.label }}
        <span v-if="tab.count" class="openapi-tab-count">{{ tab.count }}</span>
      </slot>
    </button>
  </div>
</template>
