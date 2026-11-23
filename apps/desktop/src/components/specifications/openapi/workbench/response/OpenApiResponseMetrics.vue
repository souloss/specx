<script setup lang="ts">
import { ArrowDown, ArrowUp, Globe, GlobeLock } from "@lucide/vue";
import { computed } from "vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import OpenApiResponseMetric from "./OpenApiResponseMetric.vue";
import { translate as t } from "@/i18n";
import { formatResponseBytes, formatResponseDuration, type OpenApiResponseTelemetry, type ResponseSizeMeasurement } from "@/lib/specifications/openapi/workbench/response/openapiResponseTelemetry";

const props = defineProps<{ status: number; statusText: string; metrics: OpenApiResponseTelemetry }>();
const sizeGroups = computed(() => [
  { key: "response", icon: ArrowDown, label: t("specification.openapiResponseMetrics.responseSize"), size: props.metrics.responseSize },
  { key: "request", icon: ArrowUp, label: t("specification.openapiResponseMetrics.requestSize"), size: props.metrics.requestSize },
]);
const networkRows = computed(() => [
  [t("specification.openapiResponseMetrics.transport"), t(props.metrics.transport === "agent" ? "requestAgent.agent" : "specification.openapiResponseMetrics.browserFetch")],
  [t("specification.openapiResponseMetrics.httpVersion"), props.metrics.protocol],
  [t("specification.openapiResponseMetrics.target"), new URL(props.metrics.url).host],
  [t("specification.openapiResponseMetrics.localAddress"), props.metrics.network?.localIp ? `${props.metrics.network.localIp}:${props.metrics.network.localPort}` : null],
  [t("specification.openapiResponseMetrics.remoteAddress"), props.metrics.network?.remoteIp ? `${props.metrics.network.remoteIp}:${props.metrics.network.remotePort}` : null],
  [t("specification.openapiResponseMetrics.tlsProtocol"), tlsValue(props.metrics.network?.tls?.protocol)],
  [t("specification.openapiResponseMetrics.cipher"), tlsValue(props.metrics.network?.tls?.cipher)],
  [t("specification.openapiResponseMetrics.certificate"), tlsValue(props.metrics.network?.tls?.certificateCommonName)],
  [t("specification.openapiResponseMetrics.issuer"), tlsValue(props.metrics.network?.tls?.issuerCommonName)],
  [t("specification.openapiResponseMetrics.expires"), tlsValue(props.metrics.network?.tls?.expiresAt)],
  [t("specification.openapiResponseMetrics.connection"), props.metrics.encrypted ? "HTTPS" : "HTTP"],
  [t("specification.openapiResponseMetrics.redirected"), props.metrics.redirected ? t("common.yes") : t("common.no")],
]);
function tlsValue(value: string | null | undefined) {
  if (!props.metrics.encrypted) return t("specification.openapiResponseMetrics.notApplicable");
  return value || t(props.metrics.transport === "agent" ? "specification.openapiResponseMetrics.notCollected" : "specification.openapiResponseMetrics.unavailable");
}
function total(size: ResponseSizeMeasurement) {
  return `${size.estimated ? "≈ " : ""}${formatResponseBytes(size.total)}${size.partial ? " *" : ""}`;
}
function bar(duration: number | null, index: number) {
  const prior = props.metrics.steps.slice(0, index).reduce((sum, step) => sum + (step.duration ?? 0), 0);
  const total = Math.max(props.metrics.totalTime, prior + (duration ?? 0), 1);
  return { left: `${(prior / total) * 100}%`, width: `${((duration ?? 0) / total) * 100}%` };
}
</script>

<template>
  <div class="response-metadata" :aria-label="t('specification.openapiResponseMetrics.summary')">
    <AppTooltip :text="`${status} ${statusText}`.trim()">
      <span class="response-status-badge" :class="{ failure: status >= 400, redirect: status >= 300 && status < 400 }" role="status"
        >{{ status }}<span class="sr-only"> {{ statusText }}</span></span
      >
    </AppTooltip>
    <span class="response-metric-separator" aria-hidden="true">·</span>
    <OpenApiResponseMetric kind="time" :label="t('specification.openapiResponseMetrics.timeDetails')">
      {{ formatResponseDuration(metrics.totalTime) }}
      <template #details>
        <div class="response-metric-heading">
          <strong>{{ t("specification.openapiResponseMetrics.time") }}</strong
          ><span>{{ formatResponseDuration(metrics.totalTime, true) }}</span>
        </div>
        <div v-for="(step, index) in metrics.steps" :key="step.phase" class="response-timing-row" :class="{ unavailable: step.duration === null }">
          <span>{{ t(`specification.openapiResponseMetrics.phases.${step.phase}`) }}</span>
          <div class="response-timing-track" aria-hidden="true"><span v-if="step.duration !== null" :style="bar(step.duration, index)" /></div>
          <span>{{ step.duration === null ? t("specification.openapiResponseMetrics.unavailable") : formatResponseDuration(step.duration, true) }}</span>
        </div>
        <p class="response-metric-note">{{ t(metrics.transport === "agent" ? "requestAgent.timingSource" : metrics.detailed ? "specification.openapiResponseMetrics.timingSource" : "specification.openapiResponseMetrics.timingRestricted") }}</p>
      </template>
    </OpenApiResponseMetric>
    <span class="response-metric-separator" aria-hidden="true">·</span>
    <OpenApiResponseMetric kind="size" :label="t('specification.openapiResponseMetrics.sizeDetails')">
      {{ total(metrics.responseSize) }}
      <template #details>
        <section v-for="group in sizeGroups" :key="group.key" class="response-size-group">
          <div class="response-metric-heading">
            <strong><component :is="group.icon" class="size-3.5" aria-hidden="true" />{{ group.label }}</strong
            ><span>{{ total(group.size) }}</span>
          </div>
          <dl class="response-metric-values">
            <div>
              <dt>Header</dt>
              <dd>{{ group.size.estimated ? "≈ " : "" }}{{ formatResponseBytes(group.size.headers) }}</dd>
            </div>
            <div>
              <dt>Body</dt>
              <dd>{{ group.size.body === null ? t("specification.openapiResponseMetrics.unavailable") : formatResponseBytes(group.size.body) }}</dd>
            </div>
          </dl>
        </section>
        <dl v-if="metrics.decodedBodySize !== metrics.responseSize.body" class="response-metric-values response-size-extra">
          <div>
            <dt>{{ t("specification.openapiResponseMetrics.decodedBody") }}</dt>
            <dd>{{ formatResponseBytes(metrics.decodedBodySize) }}</dd>
          </div>
        </dl>
        <p v-if="metrics.responseSize.estimated" class="response-metric-note">{{ t("specification.openapiResponseMetrics.sizeEstimated") }}</p>
        <p v-if="metrics.responseSize.partial || metrics.requestSize.partial" class="response-metric-note">{{ t("specification.openapiResponseMetrics.sizeIncomplete") }}</p>
        <p v-if="metrics.responseSize.partial" class="response-metric-note">{{ t("specification.openapiResponseMetrics.sizePartial", { size: formatResponseBytes(metrics.capturedBodySize) }) }}</p>
        <p v-if="metrics.cached" class="response-metric-note">{{ t("specification.openapiResponseMetrics.cached") }}</p>
      </template>
    </OpenApiResponseMetric>
    <span class="response-metric-separator" aria-hidden="true">·</span>
    <OpenApiResponseMetric kind="network" :label="t('specification.openapiResponseMetrics.network')">
      <component :is="metrics.encrypted ? GlobeLock : Globe" class="size-3.5" aria-hidden="true" />
      <template #details>
        <div class="response-metric-heading">
          <strong>{{ t("specification.openapiResponseMetrics.network") }}</strong>
        </div>
        <dl class="response-metric-values">
          <div v-for="[label, value] in networkRows" :key="label ?? ''">
            <dt>{{ label }}</dt>
            <dd :class="{ unavailable: !value }">{{ value || t("specification.openapiResponseMetrics.unavailable") }}</dd>
          </div>
        </dl>
        <p class="response-metric-note">{{ t(metrics.transport === "agent" ? "requestAgent.networkSource" : "specification.openapiResponseMetrics.networkRestricted") }}</p>
      </template>
    </OpenApiResponseMetric>
  </div>
</template>
