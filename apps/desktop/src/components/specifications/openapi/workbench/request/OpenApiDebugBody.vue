<script setup lang="ts">
import { computed } from "vue";
import { translate as t } from "@/i18n";
const mediaType = defineModel<string>({ required: true });
const props = defineProps<{ disabled?: boolean; availableMediaTypes?: string[] }>();
const standardFormats = [
  { value: "", label: "none" },
  { value: "multipart/form-data", label: "form-data" },
  { value: "application/x-www-form-urlencoded", label: "x-www-form-urlencoded" },
  { value: "application/json", label: "JSON" },
  { value: "application/xml", label: "XML" },
  { value: "text/plain", label: "Text" },
  { value: "application/octet-stream", label: "Binary" },
  { value: "application/yaml", label: "YAML" },
  { value: "text/html", label: "HTML" },
];
const formats = computed(() => {
  const values = new Map(standardFormats.map((format) => [format.value, format]));
  for (const value of props.availableMediaTypes ?? []) {
    if (value && !values.has(value)) values.set(value, { value, label: value });
  }
  if (mediaType.value && !values.has(mediaType.value)) values.set(mediaType.value, { value: mediaType.value, label: mediaType.value });
  return [...values.values()];
});
</script>
<template>
  <div class="debug-body-editor">
    <div class="debug-body-formats" role="group" :aria-label="t('specification.openapiMediaType')">
      <label v-for="format in formats" :key="format.value" :class="{ active: mediaType === format.value }"
        ><input v-model="mediaType" type="radio" name="openapi-body-format" :value="format.value" :disabled="disabled" /><span>{{ format.label }}</span></label
      >
    </div>
    <div v-if="mediaType" class="debug-body-content">
      <div class="debug-body-content-toolbar">
        <span class="debug-body-type-label">{{ mediaType }}</span
        ><slot name="tools" />
      </div>
      <slot />
    </div>
    <p v-else class="debug-body-none">{{ t("specification.openapiEmptyRequestBody") }}</p>
  </div>
</template>
