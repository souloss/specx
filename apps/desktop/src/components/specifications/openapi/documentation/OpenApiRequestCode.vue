<script setup lang="ts">
import { computed, ref } from "vue";
import { Check, Clipboard, Download } from "@lucide/vue";
import OpenApiSnippetSelector from "./OpenApiSnippetSelector.vue";
import { snippetFileName } from "@/lib/specifications/openapi/snippets/openapiSnippetClients";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { buildCodeSnippet, type SnippetLanguage } from "@/lib/specifications/openapi/snippets/openapiCodegen";
import { operationExampleRequest, type OpenApiRequestExampleSelection } from "@/lib/specifications/openapi/documentation/openapiDocumentation";
import type { OpenApiObject, OpenApiOperationEntry } from "@/lib/specifications/openapi/contract/openapi";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";
import { translate as t } from "@/i18n";
const props = defineProps<{ allowExport?: boolean; document: OpenApiObject | null; entry: OpenApiOperationEntry; selection?: OpenApiRequestExampleSelection }>();
const language = ref<SnippetLanguage>("curl");
const { copy: copyText, copied, failed } = useClipboardFeedback();
const code = computed(() => buildCodeSnippet(operationExampleRequest(props.document, props.entry, props.selection), language.value));
function copy() {
  void copyText(code.value);
}
function download() {
  const url = URL.createObjectURL(new Blob([code.value], { type: "text/plain;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = snippetFileName(language.value);
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
</script>

<template>
  <section class="my-5 min-w-0 overflow-hidden rounded-xl border border-border bg-card" :aria-label="t('openapiPublication.requestExample')">
    <div class="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-2">
      <h4 class="text-sm font-semibold">{{ t("openapiPublication.requestExample") }}</h4>
      <div class="flex min-w-0 flex-wrap items-center gap-2">
        <OpenApiSnippetSelector v-model="language" />
        <button type="button" class="icon-button" :aria-label="t('specification.openapiCopySnippet')" @click="copy"><Check v-if="copied" class="size-4" aria-hidden="true" /><Clipboard v-else class="size-4" aria-hidden="true" /></button>
        <button v-if="allowExport !== false" type="button" class="icon-button" :aria-label="t('specification.openapiDownloadSnippet')" @click="download"><Download class="size-4" aria-hidden="true" /></button>
      </div>
    </div>
    <OpenApiCodeSample :text="code" :language="language" />
    <p v-if="copied || failed" role="status" class="px-3 py-2 text-xs text-muted-foreground">{{ t(failed ? "specification.openapiCopyFailed" : "specification.openapiCopied") }}</p>
    <p class="border-t border-border px-3 py-2 text-xs leading-5 text-muted-foreground">{{ t("openapiPublication.requestExampleHelp") }}</p>
  </section>
</template>
