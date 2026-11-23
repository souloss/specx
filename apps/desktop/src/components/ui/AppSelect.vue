<script setup lang="ts" generic="T extends string | number | boolean | null = string">
import { computed, nextTick, ref, useAttrs, useId, useSlots, type VNode, type HTMLAttributes } from "vue";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

// Keep native option syntax and form/change semantics at call sites, while all
// visible selectors share Reka's keyboard, focus, positioning and theme behavior.
defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{ modelValue?: T; value?: T; disabled?: boolean; name?: string; id?: string; placeholder?: string; size?: "form" | "compact" | "dense"; variant?: "default" | "quiet" | "icon"; contentClass?: HTMLAttributes["class"]; modelModifiers?: { number?: boolean; trim?: boolean } }>(),
  {
    size: "compact",
    variant: "default",
  },
);
const emit = defineEmits<{ "update:modelValue": [value: T]; "update:open": [value: boolean]; optionPreview: [value: string]; optionLeave: []; change: [event: Event] }>();
const attrs = useAttrs();
const slots = useSlots();
const native = ref<HTMLSelectElement>();
const generatedId = useId();
const triggerId = computed(() => props.id ?? generatedId);
const current = computed(() => {
  const controlled = props.modelValue !== undefined || props.value !== undefined;
  if (controlled) return String(props.modelValue ?? props.value ?? "");
  return options()[0]?.value ?? "";
});
const encoded = computed(() => `option:${current.value}`);

type Option = { value: string; emittedValue: unknown; label: string; disabled: boolean; group?: string };
function text(value: unknown): string {
  if (typeof value === "string" || typeof value === "number") return String(value);
  if (Array.isArray(value)) return value.map(text).join("");
  if (value && typeof value === "object" && "children" in value) return text((value as VNode).children);
  return "";
}
function readOptions(nodes: VNode[], group?: string, disabled = false): Option[] {
  return nodes.flatMap((node): Option[] => {
    if (node.type === "option") {
      const label = text(node.children);
      const hasValue = Object.hasOwn(node.props ?? {}, "value");
      const emittedValue = hasValue ? node.props?.value : label;
      return [{ value: String(emittedValue ?? ""), emittedValue, label, group, disabled: disabled || (node.props?.disabled !== undefined && node.props.disabled !== false) }];
    }
    if (Array.isArray(node.children)) return readOptions(node.children as VNode[], node.type === "optgroup" ? String(node.props?.label ?? "") : group, disabled || (node.props?.disabled !== undefined && node.props.disabled !== false));
    return [];
  });
}
function options() {
  return readOptions(slots.default?.() ?? []);
}
function nativeChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  const option = options().find((item) => item.value === value);
  const rawValue = option?.emittedValue;
  const converted = props.modelModifiers?.number || typeof props.modelValue === "number" || typeof props.value === "number" ? Number(value) : props.modelModifiers?.trim ? value.trim() : rawValue;
  emit("update:modelValue", converted as T);
  emit("change", event);
}
async function choose(value: unknown) {
  if (typeof value !== "string" || !native.value) return;
  native.value.value = value.slice(7);
  native.value.dispatchEvent(new Event("change", { bubbles: true }));
  // Uncontrolled action selectors may intentionally retain their placeholder.
  await nextTick();
  if (native.value) native.value.value = current.value;
}
</script>

<template>
  <Select :model-value="encoded" :disabled="props.disabled" @update:model-value="choose" @update:open="emit('update:open', $event)">
    <SelectTrigger :id="triggerId" v-bind="attrs" class="app-select" :data-app-select-size="props.size" :data-app-select-variant="props.variant" :disabled="props.disabled"
      ><SelectValue
        ><slot name="selected" :value="current" :label="options().find((option) => option.value === current)?.label ?? current">{{ options().find((option) => option.value === current)?.label ?? (current || props.placeholder) }}</slot></SelectValue
      ></SelectTrigger
    >
    <SelectContent :class="['app-select-content', props.contentClass]" :side-offset="4" @pointerleave="emit('optionLeave')">
      <template v-for="(option, index) in options()" :key="option.value">
        <div v-if="option.group && options()[index - 1]?.group !== option.group" class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{{ option.group }}</div>
        <SelectItem :value="`option:${option.value}`" :disabled="option.disabled" @pointerenter="!option.disabled && emit('optionPreview', option.value)" @focus="!option.disabled && emit('optionPreview', option.value)"
          ><slot name="option" :value="option.value" :label="option.label">{{ option.label }}</slot></SelectItem
        >
      </template>
    </SelectContent>
  </Select>
  <select ref="native" :value="current" :name="props.name" :disabled="props.disabled" aria-hidden="true" tabindex="-1" class="app-select-native" @change="nativeChange">
    <slot />
  </select>
</template>

<style>
.app-select.specx-select-trigger {
  max-width: 100%;
  min-width: 0;
  min-height: 32px;
  height: 32px;
  border: 1px solid var(--input);
  border-radius: var(--specx-radius-md, 6px);
  background: var(--background);
  color: var(--foreground);
  padding: 5px 8px;
  font-size: 12px;
  line-height: 20px;
}
.app-select.specx-select-trigger[data-app-select-size="form"] {
  min-height: 36px;
  height: 36px;
  padding: 7px 9px;
  font-size: 13px;
  line-height: 20px;
}
.app-select.specx-select-trigger[data-app-select-size="dense"] {
  min-height: 28px;
  height: 28px;
  padding: 3px 7px;
  font-size: 12px;
  line-height: 18px;
}
.app-select.specx-select-trigger[data-app-select-variant="quiet"] {
  border-color: transparent;
  background: transparent;
  box-shadow: none;
  padding-inline: 6px;
}
.app-select.specx-select-trigger[data-app-select-variant="quiet"]:hover {
  border-color: var(--border);
  background: var(--background);
}
.app-select.specx-select-trigger[data-app-select-variant="quiet"]:focus-visible {
  border-color: var(--ring);
  background: var(--background);
}
.app-select.specx-select-trigger[data-app-select-variant="icon"] {
  width: 34px;
  min-width: 34px;
  justify-content: center;
  padding: 0;
}
.app-select.specx-select-trigger:hover {
  background: var(--accent);
}
.app-select.specx-select-trigger:focus-visible {
  border-color: var(--ring);
}
.app-select > [data-slot="select-value"] {
  overflow: hidden;
  text-overflow: ellipsis;
}
.app-select-native {
  display: none !important;
}
.app-select-content {
  max-width: min(36rem, calc(100vw - 1rem));
}
.app-select-content [data-slot="select-item"] {
  min-height: 32px;
  white-space: normal;
  overflow-wrap: anywhere;
}
</style>
