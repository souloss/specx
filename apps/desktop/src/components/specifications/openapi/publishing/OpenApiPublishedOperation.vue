<script setup lang="ts">
import { computed, defineAsyncComponent, ref, useId, watch } from "vue";
import OpenApiOperationDocument from "../documentation/OpenApiOperationDocument.vue";
import OpenApiOperationTabs from "../OpenApiOperationTabs.vue";
import type { OpenApiObject, OpenApiOperationEntry } from "@/lib/specifications/openapi/contract/openapi";
import { readPublicEnvironments } from "@/lib/specifications/openapi/environments/openapiPublicEnvironments";
import { translate as t } from "@/i18n";

const props = defineProps<{ allowExport?: boolean; content: string; document: OpenApiObject | null; entry: OpenApiOperationEntry; title: string; version: number; allowTryIt: boolean; navigateModel: (reference: string) => void }>();
// The public reference has no workspace dependency. Load the isolated workbench
// only after a reader explicitly opens it, keeping its request state on tab switches.
const Workbench = defineAsyncComponent(() => import("../OpenApiPreview.vue").then((module) => module.default));
const publicEnvironments = computed(() => readPublicEnvironments(props.document));
const view = ref<"docs" | "debug">("docs");
const debugVisited = ref(false);
const id = useId();
watch(
  () => props.allowTryIt,
  (allowed) => {
    if (!allowed) {
      view.value = "docs";
      debugVisited.value = false;
    }
  },
);
function select(value: string) {
  if (value !== "docs" && value !== "debug") return;
  view.value = value;
  if (value === "debug") debugVisited.value = true;
}
const operationTabs = computed(() => [
  { value: "debug", id: `${id}-debug-tab`, label: t("specification.openapiDebugTab") },
  { value: "docs", id: `${id}-docs-tab`, label: t("specification.openapiDocumentationTab") },
]);
</script>

<template>
  <article class="openapi-operation-page">
    <OpenApiOperationTabs v-if="allowTryIt" :model-value="view" :tabs="operationTabs" :ariaLabel="t('specification.openapiOperationViews')" @update:model-value="select" />
    <section :id="`${id}-docs`" v-show="view === 'docs'" class="openapi-documentation-view" :role="allowTryIt ? 'tabpanel' : undefined" :aria-labelledby="allowTryIt ? `${id}-docs-tab` : undefined" :tabindex="allowTryIt ? 0 : undefined">
      <OpenApiOperationDocument :allow-export="allowExport" :document="document" :entry="entry" :specification-title="title" :specification-version="version" :navigate-model="navigateModel" show-examples />
    </section>
    <section v-if="allowTryIt" :id="`${id}-debug`" v-show="view === 'debug'" role="tabpanel" :aria-labelledby="`${id}-debug-tab`" tabindex="0">
      <Workbench v-if="debugVisited" :content="content" :focus-operation-key="entry.identityKey" :public-environments="publicEnvironments" reader public-try-it embedded-workbench />
    </section>
  </article>
</template>
