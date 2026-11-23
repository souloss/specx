<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Clipboard, Download } from "@lucide/vue";
import DOMPurify from "dompurify";
import OpenApiResponseViewControls from "./OpenApiResponseViewControls.vue";
import OpenApiResponseBody from "./OpenApiResponseBody.vue";
import { decodeResponseBytes, responseEncodingForContentType, type ResponseEncoding, type ResponseFormat } from "@/lib/specifications/openapi/workbench/response/openapiResponsePresentation";
import { translate as t } from "@/i18n";
const props = defineProps<{ bytes?: Uint8Array; body: string; contentType: string; image?: string; binary?: string; visualize?: string }>();
const emit = defineEmits<{ copy: [text: string]; download: []; visualizationDemo: [] }>();
const mode = ref("pretty");
const format = ref<ResponseFormat>("auto");
const encoding = ref<ResponseEncoding>("utf8");
watch(
  () => props.bytes,
  () => {
    mode.value = "pretty";
    format.value = "auto";
    encoding.value = responseEncodingForContentType(props.contentType);
  },
  { immediate: true },
);
const decoded = computed(() => (props.bytes ? decodeResponseBytes(props.bytes, encoding.value) : props.body));
const syntax = computed(() => (format.value !== "auto" ? format.value : props.contentType.includes("json") ? "json" : props.contentType.includes("html") ? "html" : props.contentType.includes("xml") ? "xml" : /^[\s]*[[{]/u.test(decoded.value) ? "json" : "text"));
const display = computed(() => {
  if (mode.value !== "pretty" || ["hex", "base64", "binary"].includes(encoding.value)) return decoded.value;
  if (syntax.value === "json") {
    try {
      return JSON.stringify(JSON.parse(decoded.value), null, 4);
    } catch {
      return decoded.value;
    }
  }
  if (syntax.value === "xml" || syntax.value === "html") {
    let level = 0;
    return decoded.value
      .replace(/>\s+</gu, "><")
      .replace(/></gu, ">\n<")
      .split("\n")
      .map((line) => {
        if (line.startsWith("</")) level = Math.max(0, level - 1);
        const value = "  ".repeat(level) + line;
        if (/^<[^!?/][^>]*[^/]?>$/u.test(line) && !/<\/[^>]+>$/u.test(line)) level++;
        return value;
      })
      .join("\n");
  }
  return decoded.value;
});
const json = computed(() => mode.value === "pretty" && syntax.value === "json" && !["hex", "base64", "binary"].includes(encoding.value));
// Keep a leading <style> in the sanitized fragment instead of discarding it with the parsed head.
const previewHtml = computed(() => `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'">${DOMPurify.sanitize(mode.value === "visualize" ? (props.visualize ?? "") : decoded.value, { FORCE_BODY: true })}`);
const filtered = ref<string | null>(null);
</script>
<template>
  <div class="response-presentation">
    <div v-if="mode === 'preview' || mode === 'visualize'" class="response-presentation-toolbar">
      <OpenApiResponseViewControls v-model:mode="mode" v-model:format="format" v-model:encoding="encoding" :syntax="syntax" />
      <span class="flex-1" /><button type="button" class="icon-button" :aria-label="t('specification.openapiCopyResponse')" @click="emit('copy', filtered ?? display)"><Clipboard class="size-3.5" /></button
      ><button type="button" class="icon-button" :aria-label="t('specification.openapiDownloadResponse')" @click="emit('download')"><Download class="size-3.5" /></button>
    </div>
    <template v-if="mode === 'preview' || mode === 'visualize'"
      ><div v-if="mode === 'visualize' && !visualize" class="response-view-empty response-visualization-help">
        <p>{{ t("specification.openapiResponseVisualizeHelp") }}</p>
        <button type="button" class="debug-add-button" @click="emit('visualizationDemo')">{{ t("specification.openapiVisualizationDemo") }}</button>
      </div>
      <img v-else-if="image && mode === 'preview'" :src="image" :alt="t('specification.openapiResponseImageAlt')" class="response-preview-image" /><iframe
        v-else-if="syntax === 'html' || mode === 'visualize'"
        :srcdoc="previewHtml"
        sandbox=""
        referrerpolicy="no-referrer"
        :title="t('specification.openapiResponsePreview')"
        class="response-preview-frame"
      />
      <pre v-else class="response-body">{{ decoded }}</pre>
    </template>
    <OpenApiResponseBody v-else v-model:filtered-text="filtered" :text="display" :json="json" :syntax="mode === 'raw' ? 'text' : syntax" :can-format="false" :original="mode === 'raw'" :filter-source="json ? decoded : undefined" compact>
      <template #leading><OpenApiResponseViewControls v-model:mode="mode" v-model:format="format" v-model:encoding="encoding" :syntax="syntax" /></template>
      <template #actions>
        <button type="button" class="icon-button" :aria-label="t('specification.openapiCopyResponse')" @click="emit('copy', filtered ?? display)"><Clipboard class="size-3.5" /></button>
        <button type="button" class="icon-button" :aria-label="t('specification.openapiDownloadResponse')" @click="emit('download')"><Download class="size-3.5" /></button>
      </template>
    </OpenApiResponseBody>
  </div>
</template>
