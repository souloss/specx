<script setup lang="ts">
import { ArrowDown, ArrowUp, HelpCircle, Plus, Trash2 } from "@lucide/vue";
import OpenApiScriptEditor from "./OpenApiScriptEditor.vue";
import { Button } from "@/components/ui/button";

type ScriptSnippet = { label: string; value: string };

const props = defineProps<{
  modelValue: string[];
  phase: "before" | "after";
  label: string;
  addLabel: string;
  helpLabel: string;
  moveUpLabel: string;
  moveDownLabel: string;
  removeLabel: string;
  snippetsLabel: string;
  snippets: ScriptSnippet[];
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string[]];
  help: [];
}>();

function update(index: number, value: string) {
  const next = [...props.modelValue];
  next[index] = value;
  emit("update:modelValue", next);
}

function add() {
  emit("update:modelValue", [...props.modelValue, ""]);
}

function remove(index: number) {
  const next = props.modelValue.filter((_, itemIndex) => itemIndex !== index);
  emit("update:modelValue", next);
}

function move(index: number, direction: -1 | 1) {
  const target = index + direction;
  if (target < 0 || target >= props.modelValue.length) return;
  const next = [...props.modelValue];
  [next[index], next[target]] = [next[target]!, next[index]!];
  emit("update:modelValue", next);
}

function appendSnippet(index: number, snippet: ScriptSnippet) {
  const current = props.modelValue[index] ?? "";
  update(index, current ? `${current.trimEnd()}\n\n${snippet.value}` : snippet.value);
}
</script>

<template>
  <div class="openapi-script-sequence" :data-phase="props.phase">
    <div class="openapi-script-sequence-footer">
      <Button type="button" size="sm" variant="outline" @click="add"><Plus class="size-3.5" />{{ props.addLabel }}</Button>
      <Button type="button" size="sm" variant="ghost" @click="emit('help')"><HelpCircle class="size-3.5" />{{ props.helpLabel }}</Button>
    </div>
    <div v-for="(script, index) in props.modelValue" :key="`${props.phase}-${index}`" class="openapi-script-item">
      <div class="openapi-script-item-heading">
        <div class="openapi-script-item-title">
          <span class="openapi-script-order">{{ index + 1 }}</span>
          <strong>{{ props.label }} {{ index + 1 }}</strong>
        </div>
        <div class="openapi-script-item-actions">
          <button type="button" class="icon-button" :disabled="index === 0" :aria-label="`${props.label} ${index + 1} ${props.moveUpLabel}`" @click="move(index, -1)"><ArrowUp class="size-3.5" /></button>
          <button type="button" class="icon-button" :disabled="index === props.modelValue.length - 1" :aria-label="`${props.label} ${index + 1} ${props.moveDownLabel}`" @click="move(index, 1)"><ArrowDown class="size-3.5" /></button>
          <button type="button" class="icon-button" :aria-label="`${props.label} ${index + 1} ${props.removeLabel}`" @click="remove(index)"><Trash2 class="size-3.5" /></button>
        </div>
      </div>
      <div class="openapi-script-item-body">
        <OpenApiScriptEditor :model-value="script" :label="`${props.label} ${index + 1}`" @update:model-value="update(index, $event)" />
        <div class="openapi-script-snippets" :aria-label="props.snippetsLabel">
          <button v-for="snippet in props.snippets" :key="snippet.label" type="button" class="openapi-script-snippet" @click="appendSnippet(index, snippet)">{{ snippet.label }}</button>
        </div>
      </div>
    </div>
  </div>
</template>
