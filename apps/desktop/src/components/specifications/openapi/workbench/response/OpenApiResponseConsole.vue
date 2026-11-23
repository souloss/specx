<script setup lang="ts">
import { CheckCircle2, CircleX, Terminal } from "@lucide/vue";
import type { OpenApiScriptTest } from "@/lib/specifications/openapi/workbench/scripts/openapiScriptSandbox";
import { translate as t } from "@/i18n";
defineProps<{ logs: string[]; error?: string; tests?: OpenApiScriptTest[] }>();
</script>

<template>
  <section class="response-console response-assertions" :aria-label="t('specification.openapiResponseDetails.console')">
    <template v-if="logs.length || error || tests?.length">
      <div v-if="tests?.length" class="response-script-tests" :aria-label="t('specification.openapiScriptTestResults')">
        <strong>{{ t("specification.openapiScriptTestSummary", { passed: tests.filter((test) => test.passed).length, total: tests.length }) }}</strong>
        <div v-for="(test, index) in tests" :key="index" class="response-script-test" :class="test.passed ? 'passed' : 'failed'">
          <CheckCircle2 v-if="test.passed" class="size-3.5 shrink-0" :aria-label="t('specification.openapiScriptTestPassed')" /><CircleX v-else class="size-3.5 shrink-0" :aria-label="t('specification.openapiScriptTestFailed')" />
          <div>
            <span>{{ test.name }}</span>
            <pre v-if="test.error" translate="no">{{ test.error }}</pre>
          </div>
        </div>
      </div>
      <pre v-for="(log, index) in logs" :key="index" class="response-console-line" translate="no"><span aria-hidden="true">›</span>{{ log }}</pre>
      <pre v-if="error" class="response-console-line response-console-error" role="alert">{{ error }}</pre>
    </template>
    <div v-else class="response-view-empty">
      <Terminal class="size-6" aria-hidden="true" /><strong>{{ t("specification.openapiResponseDetails.noConsole") }}</strong>
      <p>{{ t("specification.openapiResponseDetails.consoleHelp") }}</p>
    </div>
  </section>
</template>

<style scoped>
.response-script-tests {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.75rem;
}
.response-script-test {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  margin-top: 0.5rem;
}
.response-script-test.passed {
  color: var(--color-green-600, #16a34a);
}
.response-script-test.failed {
  color: var(--destructive);
}
.response-script-test pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  margin: 0.25rem 0;
}
</style>
