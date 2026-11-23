<script setup lang="ts">
import { computed, ref, useAttrs, useId, nextTick } from "vue";
import { PopoverRoot, PopoverAnchor, PopoverContent, PopoverPortal } from "reka-ui";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { dynamicCompletionEdit, dynamicCompletionOptions, dynamicCompletionToken } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicCompletion";
import { dynamicExpressionRangeAt } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicVariables";
import { translate as t } from "@/i18n";
defineOptions({ inheritAttrs: false });
const props = defineProps<{ modelValue: string; disabled?: boolean; multiline?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const input = ref<HTMLInputElement | HTMLTextAreaElement>(),
  open = ref(false),
  cursor = ref(0),
  selected = ref(0);
const attrs = useAttrs(),
  id = useId(),
  portal = useAppearancePortal();
const token = computed(() => dynamicCompletionToken(props.modelValue.slice(0, cursor.value)));
const options = computed(() => (token.value ? dynamicCompletionOptions(token.value.query).slice(0, 60) : []));
const expanded = computed(() => open.value && options.value.length > 0);
const controlAttrs = () => ({
  ...attrs,
  value: props.modelValue,
  disabled: props.disabled,
  role: props.multiline ? undefined : "combobox",
  "aria-expanded": props.multiline ? undefined : expanded.value,
  "aria-controls": expanded.value ? id : undefined,
  "aria-activedescendant": expanded.value ? `${id}-${selected.value}` : undefined,
  "aria-autocomplete": "list" as const,
  spellcheck: false,
});
function changed(event: Event) {
  if (event instanceof InputEvent && event.isComposing) return;
  const target = event.target as HTMLInputElement | HTMLTextAreaElement;
  emit("update:modelValue", target.value);
  cursor.value = target.selectionStart ?? target.value.length;
  selected.value = 0;
  const current = dynamicCompletionToken(target.value.slice(0, cursor.value));
  open.value = Boolean(current && dynamicCompletionOptions(current.query).length);
}
async function insert(expression: string, replaceExpression = false) {
  const target = input.value;
  const value = props.modelValue;
  const start = target?.selectionStart ?? value.length;
  const end = target?.selectionEnd ?? start;
  const current = start === end ? dynamicCompletionToken(value.slice(0, start)) : null;
  const range = replaceExpression && start === end ? dynamicExpressionRangeAt(value, start) : null;
  const edit = range ? { ...range, insert: expression } : current ? dynamicCompletionEdit(current.from, start, value.slice(start), expression) : { from: start, to: end, insert: expression };
  emit("update:modelValue", value.slice(0, edit.from) + edit.insert + value.slice(edit.to));
  open.value = false;
  await nextTick();
  target?.focus();
  const caret = edit.from + edit.insert.length;
  target?.setSelectionRange(caret, caret);
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing || !expanded.value) return;
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    selected.value = (selected.value + (event.key === "ArrowDown" ? 1 : -1) + options.value.length) % options.value.length;
    globalThis.document.getElementById(`${id}-${selected.value}`)?.scrollIntoView?.({ block: "nearest" });
  } else if (event.key === "Enter" && options.value[selected.value]) {
    event.preventDefault();
    void insert(options.value[selected.value]!.expression);
  } else if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    open.value = false;
  } else if (event.key === "Tab") open.value = false;
}
function selectionChanged() {
  if (input.value?.selectionStart !== cursor.value || input.value?.selectionEnd !== cursor.value) open.value = false;
}
function focus() {
  input.value?.focus();
}
defineExpose({ focus });
</script>
<template>
  <PopoverRoot :open="expanded" @update:open="open = $event"
    ><PopoverAnchor as-child
      ><div class="openapi-variable-input" :class="{ 'has-generator': $slots.default, 'is-multiline': multiline }">
        <textarea v-if="multiline" ref="input" autocomplete="off" v-bind="controlAttrs()" class="field-control" @input="changed" @compositionend="changed" @keydown="keydown" @keyup="selectionChanged" @click="selectionChanged" />
        <input v-else ref="input" type="text" autocomplete="off" v-bind="controlAttrs()" class="field-control" @input="changed" @compositionend="changed" @keydown="keydown" @keyup="selectionChanged" @click="selectionChanged" />
        <slot :insert-generated="(expression: string) => insert(expression, true)" /></div></PopoverAnchor
    ><PopoverPortal :to="portal"
      ><PopoverContent
        class="dynamic-variable-completion"
        align="start"
        :side-offset="4"
        @open-auto-focus.prevent
        @close-auto-focus.prevent
        @interact-outside="
          (event) => {
            if (event.target === input) event.preventDefault();
          }
        "
        ><div :id="id" role="listbox" :aria-label="t('specification.openapiDynamic.title')">
          <button v-for="(item, index) in options" :id="`${id}-${index}`" :key="item.name" type="button" role="option" :aria-selected="index === selected" tabindex="-1" @pointerdown.prevent @click="insert(item.expression)">
            <code>${{ item.name }}</code>
          </button>
        </div></PopoverContent
      ></PopoverPortal
    ></PopoverRoot
  >
</template>
<style>
.openapi-variable-input {
  display: flex;
  align-items: center;
  min-width: 0;
  width: 100%;
  position: relative;
}
.openapi-variable-input > input {
  flex: 1;
  min-width: 0;
  width: 0;
}
.openapi-variable-input > textarea {
  min-width: 0;
  width: 100%;
}
.openapi-variable-input.has-generator > :is(input, textarea) {
  padding-inline-end: 31px;
}
.openapi-variable-input.has-generator > .dynamic-value-trigger {
  position: absolute;
  inset-inline-end: 3px;
}
.openapi-variable-input.has-generator.is-multiline > .dynamic-value-trigger {
  inset-block-start: 3px;
}
.dynamic-variable-completion {
  z-index: 110;
  max-height: min(300px, var(--reka-popover-content-available-height));
  max-width: calc(100vw - 24px);
  width: 330px;
  overflow: auto;
  background: var(--popover);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 6px 24px #0002;
  padding: 4px;
}
.dynamic-variable-completion button {
  width: 100%;
  text-align: start;
  display: block;
  font-size: 12px;
  padding: 5px 8px;
  min-height: 28px;
  overflow-wrap: anywhere;
}
.dynamic-variable-completion button[aria-selected="true"],
.dynamic-variable-completion button:hover {
  color: var(--primary);
  background: var(--accent);
}
</style>
