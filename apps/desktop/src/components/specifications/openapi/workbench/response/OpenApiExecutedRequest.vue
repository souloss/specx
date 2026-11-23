<script setup lang="ts">
import { translate as t } from "@/i18n";
import OpenApiResponseHeaders from "./OpenApiResponseHeaders.vue";
import OpenApiResponseBody from "./OpenApiResponseBody.vue";
import { computed } from "vue";
import { formatResponseBytes } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";
const props = defineProps<{ request: { method: string; url: string; headers: Array<[string, string]>; body: string; mediaType: string; bodyBytes?: number } }>();
const json = computed(() => props.request.mediaType.includes("json"));
const body = computed(() => {
  if (json.value) {
    try {
      return JSON.stringify(JSON.parse(props.request.body), null, 4);
    } catch {
      /* Display incomplete request bytes verbatim. */
    }
  }
  return props.request.body;
});
</script>

<template>
  <section class="executed-request-view" :aria-label="t('specification.openapiExecutedRequest')">
    <div class="executed-request-block">
      <h5>{{ t("specification.openapiRequestUrl") }}:</h5>
      <div class="executed-request-target">
        <strong class="request-method-badge" :data-method="request.method.toLowerCase()">{{ request.method }}</strong
        ><code>{{ request.url }}</code>
      </div>
    </div>
    <div class="executed-request-block">
      <h5>Header:</h5>
      <OpenApiResponseHeaders :headers="request.headers" />
    </div>
    <div class="executed-request-block">
      <h5>
        Body <span v-if="request.mediaType" class="response-request-media">{{ request.mediaType }}</span>
        <span v-if="request.bodyBytes !== undefined" class="response-request-media">{{ formatResponseBytes(request.bodyBytes) }}</span>
      </h5>
      <OpenApiResponseBody v-if="request.body" :text="body" :json="json" :can-format="false" :original="false" compact minimal />
      <p v-else class="executed-request-empty">{{ t("specification.openapiEmptyRequestBody") }}</p>
    </div>
  </section>
</template>
