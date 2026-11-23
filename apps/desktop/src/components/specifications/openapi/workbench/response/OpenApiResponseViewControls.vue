<script setup lang="ts">
import AppSelect from "@/components/ui/AppSelect.vue";
import { responseEncodings, responseFormats, type ResponseEncoding, type ResponseFormat } from "@/lib/specifications/openapi/workbench/response/openapiResponsePresentation";
import { translate as t } from "@/i18n";
const mode = defineModel<string>("mode", { required: true });
const format = defineModel<ResponseFormat>("format", { required: true });
const encoding = defineModel<ResponseEncoding>("encoding", { required: true });
defineProps<{ syntax: string }>();
</script>
<template>
  <div class="response-presentation-controls">
    <div class="response-presentation-modes" role="group" :aria-label="t('specification.openapiResponseFormat')">
      <button v-for="item in ['pretty', 'raw', 'preview', 'visualize']" :key="item" type="button" :aria-pressed="mode === item" @click="mode = item">{{ item[0]!.toUpperCase() + item.slice(1) }}</button>
    </div>
    <AppSelect v-if="mode === 'pretty'" v-model="format" size="dense" variant="quiet" class="response-presentation-select" name="response-format" :aria-label="t('specification.openapiResponseSyntax')">
      <option v-for="item in responseFormats" :key="item" :value="item">{{ item === "auto" ? (syntax === "text" ? "Auto" : syntax.toUpperCase()) : item === "text" ? "Text" : item.toUpperCase() }}</option>
    </AppSelect>
    <AppSelect v-if="mode === 'pretty' || mode === 'raw'" v-model="encoding" size="dense" variant="quiet" class="response-presentation-select response-encoding-select" name="response-encoding" :aria-label="t('specification.openapiResponseEncoding')">
      <option v-for="item in responseEncodings" :key="item" :value="item">{{ item }}</option>
    </AppSelect>
  </div>
</template>
