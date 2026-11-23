<script setup lang="ts">
import { computed, ref } from "vue";
import { Download } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppSelect from "@/components/ui/AppSelect.vue";
import { Dialog, DialogDescription, DialogHeader, DialogTitle, DialogScrollContent } from "@/components/ui/dialog";
import { documentMarkdown, documentationHtml, documentationIndex, openApiYaml, operationOpenApiDocument } from "@/lib/specifications/openapi/documentation/openapiDocumentExport";
import { downloadText } from "@/lib/transfer/downloadText";
import { getOpenApiOperations, parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";

const props = defineProps<{ content: string; title: string; selection: string }>();
const emit = defineEmits<{ close: [] }>();
const format = ref("json");
const scope = ref("all");
const busy = ref(false);
const failed = ref(false);
const document = computed(() => parseOpenApiContent(props.content));
const operation = computed(() => getOpenApiOperations(props.content).find((entry) => entry.identityKey === props.selection));
async function download() {
  if (!document.value || busy.value) return;
  busy.value = true;
  failed.value = false;
  // Let the pressed button render before serializing a large document.
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  try {
    const source = scope.value === "current" && operation.value ? operationOpenApiDocument(document.value, operation.value) : document.value;
    const title = scope.value === "current" && operation.value ? `${operation.value.method.toUpperCase()} ${operation.value.path}` : props.title;
    if (format.value === "json") downloadText(JSON.stringify(source, null, 2), "openapi.json", "application/json");
    else if (format.value === "yaml") downloadText(openApiYaml(source), "openapi.yaml", "application/yaml");
    else if (format.value === "llms") downloadText(documentationIndex(source, title, location.href), "llms.txt");
    else if (format.value === "html") downloadText(documentationHtml(documentMarkdown(source, title), title), "api-documentation.html", "text/html;charset=utf-8");
    else downloadText(documentMarkdown(source, title), "api-documentation.md", "text/markdown;charset=utf-8");
  } catch {
    failed.value = true;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="true" @update:open="(open) => !open && emit('close')">
    <DialogScrollContent class="gap-4 sm:max-w-lg">
      <DialogHeader
        ><DialogTitle>{{ t("openapiPublication.export") }}</DialogTitle
        ><DialogDescription>{{ t("openapiPublication.exportHelp") }}</DialogDescription></DialogHeader
      >
      <label class="grid gap-2 text-sm"
        >{{ t("openapiPublication.exportFormat") }}
        <AppSelect v-model="format" size="form" name="document-export-format" class="w-full">
          <option value="json">OpenAPI · JSON</option>
          <option value="yaml">OpenAPI · YAML</option>
          <option value="html">{{ t("openapiPublication.exportHtml") }}</option>
          <option value="markdown">{{ t("openapiPublication.exportMarkdown") }}</option>
          <option value="llms">{{ t("openapiPublication.exportLlms") }}</option>
        </AppSelect>
      </label>
      <label v-if="operation" class="grid gap-2 text-sm"
        >{{ t("openapiPublication.exportScope") }}
        <AppSelect v-model="scope" size="form" name="document-export-scope" class="w-full"
          ><option value="all">{{ t("openapiPublication.exportAll") }}</option>
          <option value="current">{{ t("openapiPublication.exportCurrent") }} · {{ operation.method.toUpperCase() }} {{ operation.path }}</option></AppSelect
        >
      </label>
      <p class="text-xs leading-5 text-muted-foreground">{{ t("openapiPublication.exportOriginal") }}</p>
      <p v-if="failed" role="alert" class="text-xs text-destructive">{{ t("openapiPublication.exportFailed") }}</p>
      <div class="flex justify-end gap-2">
        <Button variant="outline" @click="emit('close')">{{ t("common.cancel") }}</Button
        ><Button :disabled="busy" @click="download"><Download class="size-4" aria-hidden="true" />{{ t(busy ? "common.loading" : "openapiPublication.export") }}</Button>
      </div>
    </DialogScrollContent>
  </Dialog>
</template>
