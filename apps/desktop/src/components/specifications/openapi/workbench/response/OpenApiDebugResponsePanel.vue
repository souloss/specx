<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { CheckCircle2, ChevronDown, CircleX, LoaderCircle, Rocket } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import OpenApiOperationTabs, { type OpenApiOperationTab } from "../../OpenApiOperationTabs.vue";
import OpenApiResponseMetrics from "./OpenApiResponseMetrics.vue";
import type { OpenApiResponseTelemetry } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";
import { translate as t } from "@/i18n";
const props = defineProps<{
  running: boolean;
  hasResponse: boolean;
  statusOptions: Array<{ code: string; label: string }>;
  tabs?: OpenApiOperationTab[];
  metrics?: OpenApiResponseTelemetry;
  statusCode?: number;
  statusText?: string;
  failed?: boolean;
  validation?: { ok: boolean; errors: string[] } | null;
}>();
const validate = defineModel<boolean>("validate", { default: false });
const status = defineModel<string>("status", { default: "" });
const tab = defineModel<string>("tab", { default: "body" });
const layout = ref<HTMLElement>();
const expanded = ref(true);
const sidebarPercent = ref(28);
const hasResult = computed(() => validate.value && props.hasResponse && !props.running && props.validation);
const showSidebar = computed(() => Boolean(hasResult.value && !props.validation?.ok && expanded.value));
watch(
  () => props.validation,
  () => {
    expanded.value = true;
  },
);
let stopDrag: (() => void) | undefined;
function resize(percent: number) {
  sidebarPercent.value = Math.max(18, Math.min(55, percent));
}
function drag(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.currentTarget as HTMLElement;
  const rect = layout.value?.getBoundingClientRect();
  if (!rect?.width) return;
  stopDrag?.();
  target.setPointerCapture(event.pointerId);
  const move = (e: PointerEvent) => resize(((rect.right - e.clientX) / rect.width) * 100);
  const stop = () => {
    target.removeEventListener("pointermove", move);
    target.removeEventListener("pointerup", stop);
    target.removeEventListener("lostpointercapture", stop);
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    stopDrag = undefined;
  };
  stopDrag = stop;
  target.addEventListener("pointermove", move);
  target.addEventListener("pointerup", stop);
  target.addEventListener("lostpointercapture", stop);
}
function resizeKey(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  resize(event.key === "Home" ? 18 : event.key === "End" ? 55 : sidebarPercent.value + (event.key === "ArrowLeft" ? 5 : -5));
}
onBeforeUnmount(() => stopDrag?.());
</script>
<template>
  <section class="openapi-debug-section openapi-response-section" :aria-busy="running">
    <header class="response-panel-header">
      <OpenApiOperationTabs v-if="tabs?.length && (hasResponse || failed || running)" v-model="tab" class="response-tabs" :tabs="tabs" :ariaLabel="t('specification.openapiResponseViews')" />
      <h4 v-else>{{ t("specification.openapiResponses") }}</h4>
      <div class="response-panel-header-actions">
        <OpenApiResponseMetrics v-if="metrics && statusCode !== undefined && !running" :status="statusCode" :status-text="statusText ?? ''" :metrics="metrics" />
        <div v-if="statusOptions.length" class="debug-response-actions">
          <label class="debug-response-validation"
            ><span>{{ t("specification.openapiValidateResponseLabel") }}</span
            ><input v-model="validate" type="checkbox" role="switch" name="openapi-validate-response" class="debug-switch"
          /></label>
          <AppSelect v-model="status" size="dense" class="debug-response-status-select" :aria-label="t('specification.openapiResponseStatus')">
            <option v-for="option in statusOptions" :key="option.code" :value="option.code">{{ option.label }} ({{ option.code }})</option>
          </AppSelect>
          <span v-if="hasResult && validation?.ok" class="response-validation-result success" role="status"><CheckCircle2 class="size-3" />{{ t("specification.openapiValidationPassedBadge") }}</span>
          <AppTooltip v-else-if="hasResult" :text="t('specification.openapiToggleValidationDetails')"
            ><button type="button" class="response-validation-result failure" :aria-expanded="showSidebar" @click="expanded = !expanded"><CircleX class="size-3" />{{ t("specification.openapiValidationFailedBadge") }}<ChevronDown class="size-3" :class="{ 'rotate-180': showSidebar }" /></button
          ></AppTooltip>
        </div>
        <slot name="actions" />
      </div>
    </header>
    <div ref="layout" class="response-panel-layout" :class="{ 'has-validation': showSidebar }" :style="{ '--validation-width': `${sidebarPercent}%` }">
      <div class="response-panel-main">
        <div v-if="running" class="openapi-response-empty" role="status"><LoaderCircle class="size-7 animate-spin" />{{ t("specification.openapiRequestRunning") }}</div>
        <slot v-else />
        <div v-if="!running && !hasResponse && !failed" class="openapi-response-empty">
          <span class="debug-response-empty-icon"><Rocket class="size-7" stroke-width="1.5" aria-hidden="true" /></span>{{ t("specification.openapiDebugResponseEmpty") }}
        </div>
      </div>
      <template v-if="showSidebar">
        <div
          class="response-validation-resizer"
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-label="t('specification.openapiResizeValidation')"
          :aria-valuenow="Math.round(sidebarPercent)"
          :aria-valuemin="18"
          :aria-valuemax="55"
          @pointerdown.prevent="drag"
          @keydown="resizeKey"
          @dblclick="sidebarPercent = 28"
        >
          <span />
        </div>
        <aside class="response-validation-sidebar" :aria-label="t('specification.openapiValidateResponseLabel')">
          <div class="response-validation-card" role="status">
            <h5>{{ t("specification.openapiValidateResponseLabel") }}</h5>
            <p>
              <CircleX class="size-3.5" /><span>{{ t("specification.openapiContractFailed") }}</span>
            </p>
            <ol>
              <li v-for="(error, index) in validation?.errors" :key="index">{{ error }}</li>
            </ol>
          </div>
        </aside>
      </template>
    </div>
  </section>
</template>
