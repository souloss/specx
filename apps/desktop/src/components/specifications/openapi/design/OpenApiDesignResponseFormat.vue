<script setup lang="ts">
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate as t } from "@/i18n";
import { responseFormatLabel, responseFormatPresets } from "@/lib/specifications/openapi/design/openapiResponseFormats";

const current = defineModel<string>({ required: true });
const props = defineProps<{ mediaTypes: string[]; disabled?: boolean; editable: boolean; name: string }>();
</script>

<template>
  <AppSelect v-model="current" :name="props.name" :aria-label="t('specification.openapiDesignResponseFormat')" :disabled="disabled" variant="quiet" size="dense" class="openapi-response-format-select">
    <option v-if="editable || !mediaTypes.length" value="">{{ t("specification.openapiDesignResponseFormatNone") }}</option>
    <option v-for="mediaType in mediaTypes" :key="mediaType" :value="mediaType">{{ responseFormatLabel(mediaType) }}</option>
    <template v-if="editable">
      <option v-for="[mediaType, label] in responseFormatPresets.filter(([type]) => !mediaTypes.some((item) => item.toLowerCase() === type))" :key="mediaType" :value="mediaType">{{ label }}</option>
      <option value="__custom">{{ t("specification.openapiDesignResponseFormatCustom") }}</option>
    </template>
  </AppSelect>
</template>
