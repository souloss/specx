<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { translate as t } from "@/i18n";
import { parseResponseExample, validResponseExampleName, type ResponseCaptureTarget, type ResponseExampleInput, type ResponseExampleTarget } from "@/lib/specifications/openapi/design/openapiResponseExamples";
import { inferResponseSchema } from "@/lib/specifications/openapi/design/openapiResponseInference";
import OpenApiSchemaPreview from "../shared/OpenApiSchemaPreview.vue";

const props = defineProps<{ target: ResponseExampleTarget; editingName?: string; initialBody: string; names: string[]; readonly?: boolean; capture?: { action: ResponseCaptureTarget["action"]; version?: "3.0" | "3.1" } }>();
const emit = defineEmits<{ close: []; apply: [example: ResponseExampleInput, infer: boolean] }>();
const name = ref(props.editingName ?? "");
const body = ref(props.initialBody);
const mode = ref<"example" | "schema">("example");
const submitted = ref(false);
const parsed = computed(() => parseResponseExample(body.value, props.target.mediaType));
const schema = computed(() => (mode.value === "schema" && props.capture?.version && "value" in parsed.value ? inferResponseSchema(parsed.value.value, props.capture.version) : null));
const error = computed(() => {
  if (props.editingName === undefined) {
    if (!validResponseExampleName(name.value.trim())) return t("specification.openapiResponseExample.invalidName");
    if (props.names.includes(name.value.trim())) return t("specification.openapiResponseExample.duplicate");
  }
  if ("error" in parsed.value) return t(`specification.openapiResponseExample.${parsed.value.error}`);
  return mode.value === "schema" && !schema.value ? t("specification.openapiResponseExample.schemaTooComplex") : "";
});

function apply() {
  if (props.readonly) return;
  submitted.value = true;
  if (!error.value) emit("apply", { name: props.editingName ?? name.value.trim(), body: body.value }, mode.value === "schema");
}
</script>

<template>
  <Dialog :open="true" @update:open="(open) => !open && emit('close')">
    <DialogScrollContent class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 sm:max-w-xl">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle>{{ t(props.readonly ? "specification.openapiDesignViewExample" : props.capture ? "specification.openapiResponseExample.extract" : props.editingName === undefined ? "specification.openapiResponseExample.add" : "specification.openapiResponseExample.edit") }}</DialogTitle>
        <DialogDescription v-if="props.capture">{{ t("specification.openapiResponseExample.review") }}</DialogDescription>
        <p class="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
          <span v-if="props.capture">{{ t(`specification.openapiResponseExample.${props.capture.action}`) }}</span>
          <template v-if="props.target.status"
            ><code>{{ props.target.status }}</code
            ><span aria-hidden="true">·</span></template
          ><code class="min-w-0 wrap-anywhere">{{ props.target.mediaType }}</code>
        </p>
      </DialogHeader>
      <form id="openapi-response-example-form" class="min-h-0 space-y-3 overflow-y-auto overscroll-contain px-5 py-4" novalidate @submit.prevent="apply">
        <fieldset v-if="props.capture?.version" class="space-y-1.5 text-sm">
          <legend class="mb-1.5">{{ t("specification.openapiResponseExample.include") }}</legend>
          <div class="flex flex-wrap gap-2">
            <label v-for="option in ['example', 'schema'] as const" :key="option" class="flex cursor-pointer items-center gap-2 rounded-md border border-border px-2.5 py-1.5 has-checked:border-primary/50 has-checked:bg-primary/5">
              <input v-model="mode" type="radio" name="openapi-response-capture-mode" :value="option" class="accent-primary" />
              {{ t(option === "example" ? "specification.openapiResponseExample.exampleOnly" : "specification.openapiResponseExample.schemaAndExample") }}
            </label>
          </div>
        </fieldset>
        <label v-if="props.editingName === undefined" class="grid gap-1.5 text-sm">
          {{ t("specification.openapiResponseExample.name") }}
          <input v-model="name" name="openapi-example-name" class="field-control" maxlength="64" autocomplete="off" spellcheck="false" />
        </label>
        <label class="grid gap-1.5 text-sm">
          {{ t("specification.openapiResponseExample.body") }}
          <textarea v-model="body" name="openapi-example-body" class="field-control min-h-36 w-full resize-y font-mono text-xs leading-relaxed" :readonly="props.readonly" spellcheck="false" />
        </label>
        <section v-if="mode === 'schema'" class="space-y-1.5 rounded-md border border-border p-3">
          <h3 class="text-sm font-medium">{{ t("specification.openapiResponseExample.schemaPreview") }}</h3>
          <p class="text-xs text-muted-foreground">{{ t("specification.openapiResponseExample.schemaHint") }}</p>
          <OpenApiSchemaPreview v-if="schema" :schema="schema" :document="null" />
          <p v-else-if="!submitted" class="text-xs text-muted-foreground">{{ "error" in parsed ? t(`specification.openapiResponseExample.${parsed.error}`) : t("specification.openapiResponseExample.schemaTooComplex") }}</p>
        </section>
        <p v-if="submitted && error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </form>
      <DialogFooter class="m-0 rounded-b-lg border-t border-border px-5 py-4">
        <Button variant="outline" @click="emit('close')">{{ t(props.readonly ? "common.close" : "common.cancel") }}</Button>
        <Button v-if="!props.readonly" type="submit" form="openapi-response-example-form">{{ t("specification.openapiResponseExample.apply") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
