<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate as t } from "@/i18n";

const props = defineProps<{ name: string; disabled?: boolean; invalid?: boolean; errorId?: string }>();
const model = defineModel<string>({ required: true });
const emit = defineEmits<{ input: []; submit: [] }>();
const presets = ["application/json", "application/problem+json", "application/xml", "text/plain", "text/html", "application/x-www-form-urlencoded", "multipart/form-data", "application/octet-stream", "application/pdf", "text/event-stream"];
const selection = computed({
  get: () => (presets.includes(model.value) ? model.value : ""),
  set: (value: string) => {
    model.value = value;
    emit("input");
  },
});
</script>

<template>
  <div class="openapi-media-type-input">
    <input
      v-model="model"
      class="field-control field-control-compact"
      :name="props.name"
      :aria-label="t('specification.openapiDesignMediaType')"
      :aria-invalid="props.invalid"
      :aria-describedby="props.invalid ? props.errorId : undefined"
      :placeholder="t('specification.openapiDesignMediaTypePlaceholder')"
      :disabled="disabled"
      autocomplete="off"
      spellcheck="false"
      @input="emit('input')"
      @keydown.enter.prevent="emit('submit')"
    />
    <AppSelect v-model="selection" size="compact" variant="icon" class="media-type-presets" :name="`${name}-preset`" :aria-label="t('specification.openapiMediaTypePresets')" :disabled="disabled">
      <template #selected><ChevronDown class="size-4" aria-hidden="true" /></template>
      <option value="" disabled>{{ t("specification.openapiMediaTypePresets") }}</option>
      <option v-for="type in presets" :key="type" :value="type">{{ type }}</option>
    </AppSelect>
  </div>
</template>

<style>
.openapi-media-type-input {
  display: flex;
  flex: 1 1 16rem;
  min-width: 0;
  align-items: stretch;
}
.openapi-design-media-add .openapi-media-type-input input {
  width: 100%;
  min-width: 0;
  flex: 1 1 auto;
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}
.openapi-media-type-input .media-type-presets {
  flex: 0 0 34px;
  width: 34px;
  justify-content: center;
  padding: 0;
  border-left: 0;
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
}
.media-type-presets > [data-slot="select-icon"],
.media-type-presets > svg {
  display: none;
}
.openapi-media-type-input:focus-within {
  border-radius: var(--specx-radius-md, 6px);
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
</style>
