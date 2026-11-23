<script setup lang="ts">
import { useTemplateRef } from "vue";
import OpenApiDynamicValuePicker from "./OpenApiDynamicValuePicker.vue";
import OpenApiVariableField from "./OpenApiVariableField.vue";

withDefaults(defineProps<{ modelValue: string; disabled?: boolean; generator?: boolean; multiline?: boolean }>(), { generator: true });
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const field = useTemplateRef("field");
defineExpose({ focus: () => field.value?.focus() });
</script>

<template>
  <OpenApiVariableField ref="field" :model-value="modelValue" :disabled="disabled" :multiline="multiline" @update:model-value="emit('update:modelValue', $event)">
    <template v-if="generator !== false" #default="{ insertGenerated }">
      <OpenApiDynamicValuePicker :disabled="disabled" @insert="insertGenerated" />
    </template>
  </OpenApiVariableField>
</template>
