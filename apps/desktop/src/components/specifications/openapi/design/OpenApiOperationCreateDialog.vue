<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { LoaderCircle, Plus } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import AppSelect from "@/components/ui/AppSelect.vue";
import { OPENAPI_METHODS, formatOpenApiContent, parseOpenApiContent } from "@/lib/specifications/openapi/contract/openapi";
import { createOpenApiOperation, validateOpenApiOperationAddress, type OpenApiOperationChange } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import { translate } from "@/i18n";

const props = defineProps<{ content: string; editable: boolean; saveOperation: (content: string, change?: OpenApiOperationChange) => Promise<boolean> }>();
const open = defineModel<boolean>("open", { required: true });
const t = translate;
const method = ref("get");
const path = ref("");
const summary = ref("");
const tags = ref("");
const busy = ref(false);
const error = ref("");
const document = computed(() => parseOpenApiContent(props.content));
const validation = computed(() => (document.value && path.value.trim() ? validateOpenApiOperationAddress(document.value, { method: method.value, path: path.value }) : null));

watch(open, (value) => {
  if (!value) return;
  method.value = "get";
  path.value = "";
  summary.value = "";
  tags.value = "";
  error.value = "";
});

async function create() {
  if (!document.value || !props.editable || busy.value || !path.value.trim() || validation.value) return;
  busy.value = true;
  error.value = "";
  try {
    const next = createOpenApiOperation(document.value, { method: method.value, path: path.value, summary: summary.value, tags: tags.value.split(",") });
    const content = formatOpenApiContent(JSON.stringify(next), props.content.trimStart().startsWith("{") ? "json" : "yaml");
    const change = { key: `${method.value.toUpperCase()}:${path.value.trim()}`, title: `${method.value.toUpperCase()} ${path.value.trim()}` };
    if (await props.saveOperation(content, change)) open.value = false;
    else error.value = t("specification.openapiDesignSaveFailed");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.openapiDesignSaveFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogScrollContent class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 sm:max-w-lg">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle class="flex items-center gap-2 text-base"><Plus class="size-4" />{{ t("specification.openapiCreateOperation") }}</DialogTitle>
      </DialogHeader>
      <form id="openapi-create-operation-form" class="min-h-0 space-y-4 overflow-y-auto overscroll-contain px-5 py-4" :aria-busy="busy" @submit.prevent="create">
        <p class="text-sm text-muted-foreground">{{ t("specification.openapiCreateOperationDescription") }}</p>
        <fieldset :disabled="busy || !props.editable" class="m-0 min-w-0 space-y-4 border-0 p-0">
          <div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-3">
            <div class="space-y-2">
              <Label for="openapi-create-method">{{ t("specification.openapiOperationMethod") }}</Label>
              <AppSelect id="openapi-create-method" v-model="method" size="form" class="w-full font-mono" name="openapi-create-method">
                <option v-for="item in OPENAPI_METHODS" :key="item" :value="item">{{ item.toUpperCase() }}</option>
              </AppSelect>
            </div>
            <div class="space-y-2">
              <Label for="openapi-create-path">{{ t("specification.openapiOperationPath") }}</Label>
              <Input id="openapi-create-path" v-model="path" name="openapi-create-path" class="font-mono" placeholder="/users/{id}" autocomplete="off" :spellcheck="false" :aria-invalid="Boolean(validation)" :aria-describedby="validation ? 'openapi-create-address-error' : undefined" required />
            </div>
          </div>
          <p v-if="validation" id="openapi-create-address-error" role="alert" class="text-sm text-destructive">{{ t(`specification.openapiOperationAddressError.${validation}`) }}</p>
          <div class="space-y-2">
            <Label for="openapi-create-summary">{{ t("specification.openapiDesignSummary") }}</Label
            ><Input id="openapi-create-summary" v-model="summary" name="openapi-create-summary" autocomplete="off" />
          </div>
          <div class="space-y-2">
            <Label for="openapi-create-tags">{{ t("specification.openapiDesignTags") }}</Label
            ><Input id="openapi-create-tags" v-model="tags" name="openapi-create-tags" :placeholder="t('specification.openapiDesignTagsPlaceholder')" autocomplete="off" />
          </div>
        </fieldset>
        <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>
      </form>
      <DialogFooter class="m-0 rounded-b-lg border-t border-border px-5 py-4"
        ><Button variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button
        ><Button type="submit" form="openapi-create-operation-form" :disabled="busy || !props.editable || !document || !path.trim() || Boolean(validation)"
          ><LoaderCircle v-if="busy" class="size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /><Plus v-else class="size-3.5" aria-hidden="true" />{{ busy ? t("common.saving") : t("specification.openapiCreateOperation") }}</Button
        ></DialogFooter
      >
    </DialogScrollContent>
  </Dialog>
</template>
