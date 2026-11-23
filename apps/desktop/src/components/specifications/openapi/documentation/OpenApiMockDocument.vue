<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown, LoaderCircle } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import OpenApiOperationTabs from "../OpenApiOperationTabs.vue";
import OpenApiCodeSample from "../shared/OpenApiCodeSample.vue";
import { formatOpenApiExample, generateOpenApiExample, getOpenApiMediaExamples, isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { useWorkspaceStore } from "@/stores/workspace";
import { translate as t } from "@/i18n";

const props = defineProps<{ serviceId: string; path: string; responses: Array<[string, OpenApiObject]>; document: OpenApiObject | null }>();
const workspace = useWorkspaceStore();
const open = ref(true);
const tab = ref("local");
const mockBase = ref("");
const busy = ref(false);
const error = ref("");
const preview = ref<{ name: string; text: string; mediaType: string } | null>(null);
const tabs = computed(() => [
  { value: "local", id: "openapi-mock-local-tab", label: t("specification.openapiMockLocal") },
  { value: "cloud", id: "openapi-mock-cloud-tab", label: t("specification.openapiMockCloud") },
]);
const rows = computed(() =>
  props.responses.flatMap(([code, response]) => {
    if (!isRecord(response.content)) return [];
    const media = Object.entries(response.content).find(([, value]) => isRecord(value));
    if (!media || !isRecord(media[1])) return [];
    const examples = getOpenApiMediaExamples(props.document, media[1]);
    const samples = examples.length ? examples : [{ name: code, label: String(response.description ?? code), value: generateOpenApiExample(props.document, media[1].schema) }];
    return samples.map((example) => ({ code, name: example.label, text: formatOpenApiExample(example.value, media[0]), mediaType: media[0] }));
  }),
);
async function enableMock() {
  busy.value = true;
  error.value = "";
  try {
    const result = await workspace.enableOpenApiMock(props.serviceId);
    if (result.token) mockBase.value = `${window.location.origin}/api/specx/mock/openapi/${encodeURIComponent(result.token)}`;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("specification.openapiMockEnableFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section v-if="rows.length" class="openapi-mock-document">
    <button type="button" class="openapi-mock-heading" :aria-expanded="open" aria-controls="openapi-mock-content" @click="open = !open">{{ t("specification.openapiMockTitle") }}<ChevronDown class="size-3.5" :class="{ '-rotate-90': !open }" /></button>
    <div v-show="open" id="openapi-mock-content">
      <OpenApiOperationTabs :model-value="tab" :tabs="tabs" :ariaLabel="t('specification.openapiMockTitle')" @update:model-value="tab = $event" />
      <div v-if="tab === 'cloud' && !mockBase" class="openapi-mock-enable">
        <span>{{ t("specification.openapiMockEnableHint") }}</span
        ><Button size="sm" variant="outline" :disabled="busy" @click="enableMock"><LoaderCircle v-if="busy" class="size-3.5 animate-spin" />{{ t("specification.openapiMockEnable") }}</Button>
      </div>
      <p v-if="error" class="py-2 text-xs text-destructive" role="alert">{{ error }}</p>
      <div class="openapi-mock-table-scroll">
        <table class="openapi-mock-table">
          <thead>
            <tr>
              <th>{{ t("specification.openapiName") }}</th>
              <th>{{ t("specification.openapiMockSource") }}</th>
              <th>{{ t("specification.openapiMockUrl") }}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, index) in rows" :key="index">
              <td>
                <span class="mr-1 text-emerald-600">{{ row.code }}</span
                >{{ row.name }}
              </td>
              <td>{{ t("specification.openapiMockResponseSource") }}</td>
              <td>
                <code v-if="tab === 'cloud' && mockBase">{{ mockBase }}{{ path }}</code
                ><span v-else class="text-muted-foreground">{{ t(tab === "local" ? "specification.openapiMockSchemaSource" : "specification.openapiMockNotEnabled") }}</span>
              </td>
              <td>
                <Button size="sm" variant="outline" @click="preview = row">{{ t("specification.openapiResponsePreview") }}</Button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <Dialog :open="Boolean(preview)" @update:open="(value) => !value && (preview = null)">
      <DialogScrollContent class="gap-0 p-0 sm:max-w-2xl"
        ><DialogHeader class="border-b border-border px-5 py-4 pr-12"
          ><DialogTitle>{{ preview?.name }}</DialogTitle></DialogHeader
        ><OpenApiCodeSample v-if="preview" :text="preview.text" :language="preview.mediaType"
      /></DialogScrollContent>
    </Dialog>
  </section>
</template>
