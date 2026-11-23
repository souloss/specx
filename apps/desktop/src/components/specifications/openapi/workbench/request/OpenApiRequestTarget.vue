<script setup lang="ts">
import { ref, useId } from "vue";
import OpenApiVariableInput from "../variables/OpenApiVariableInput.vue";
import { Play, Square } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate } from "@/i18n";
import { openApiMethodClass } from "@/lib/specifications/openapi/contract/openapi";

const props = defineProps<{
  method: string;
  url: string;
  urlEditable?: boolean;
  methodEditable?: boolean;
  urlError: string;
  running: boolean;
  publicTryIt: boolean;
  unsafeServerUrl: boolean;
}>();

const emit = defineEmits<{
  "update:url": [value: string];
  "update:method": [value: string];
  send: [];
  cancel: [];
}>();

const t = translate;
const requestUrlInput = ref<{ focus: () => void }>();
const requestUrlId = useId();

function focusUrl() {
  requestUrlInput.value?.focus();
}

defineExpose({ focusUrl });
</script>

<template>
  <div class="openapi-request-setup">
    <div class="openapi-request-target">
      <div class="request-target-field request-target-method">
        <span class="request-target-label">{{ t("specification.openapiRequestMethod") }}</span>
        <AppSelect
          v-if="methodEditable"
          :model-value="props.method"
          size="compact"
          variant="quiet"
          class="request-target-method-select"
          :class="openApiMethodClass(props.method)"
          name="openapi-request-method"
          :aria-label="t('specification.openapiRequestMethod')"
          @update:model-value="emit('update:method', $event ?? 'GET')"
          ><option v-for="method in ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'TRACE']" :key="method" :value="method">{{ method }}</option></AppSelect
        >
        <code v-else class="request-target-method-value method-chip" :class="openApiMethodClass(props.method)">{{ props.method }}</code>
      </div>

      <div v-if="props.urlEditable" class="request-target-field request-target-url">
        <label :for="requestUrlId" class="request-target-label">{{ t("specification.openapiRequestUrl") }}</label>
        <slot name="prefix" />
        <OpenApiVariableInput
          :id="requestUrlId"
          ref="requestUrlInput"
          :model-value="props.url"
          :aria-invalid="Boolean(props.urlError)"
          :aria-describedby="props.urlError ? 'openapi-request-url-error' : undefined"
          class="field-control request-url-editor"
          name="openapi-request-url"
          autocomplete="off"
          autocapitalize="none"
          spellcheck="false"
          @update:model-value="emit('update:url', $event)"
        />
      </div>
      <div v-else class="request-target-field request-target-url">
        <span class="request-target-label">{{ t("specification.openapiRequestUrl") }}</span>
        <code class="request-target-url-value">{{ props.url }}</code>
      </div>

      <div class="request-target-send">
        <Button v-if="!props.running" size="sm" class="request-target-send-button" @click="emit('send')"> <Play class="size-3.5" aria-hidden="true" />{{ t("specification.openapiSendRequest") }} </Button>
        <Button v-else size="sm" variant="outline" class="request-target-send-button" @click="emit('cancel')"> <Square class="size-3.5" aria-hidden="true" />{{ t("specification.openapiCancelRequest") }} </Button>
      </div>
      <div v-if="$slots.actions" class="request-target-extra-actions"><slot name="actions" /></div>
    </div>

    <p v-if="props.urlError" id="openapi-request-url-error" role="alert" class="request-target-error">{{ props.urlError }}</p>
    <p v-if="props.publicTryIt" class="request-target-public-hint">{{ t("openapiPublication.publicTryItHint") }}</p>
    <p v-if="props.unsafeServerUrl" class="request-target-unsafe" role="alert">{{ t("specification.openapiUnsafeServer") }}</p>
  </div>
</template>

<style scoped>
.openapi-request-setup {
  container: openapi-request-setup / inline-size;
  min-width: 0;
  margin-block-end: 0;
}

.openapi-request-target {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 0;
  min-width: 0;
  padding: 10px 0 12px;
  border-bottom: 1px solid var(--border);
  background: transparent;
}

.request-target-field {
  position: relative;
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  min-height: 34px;
  background: var(--surface-muted);
}
.request-target-method {
  padding-inline: 5px;
  border: 1px solid var(--border);
  border-right: 0;
  border-radius: 5px 0 0 5px;
}
.request-target-url {
  border: 1px solid var(--border);
  border-left: 0;
  border-radius: 0 5px 5px 0;
}
.request-target-method-select {
  width: 90px;
  font-weight: 600;
}

.request-target-label {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  clip-path: inset(50%);
  white-space: nowrap;
}

.request-target-method-value,
.request-target-url-value {
  display: flex;
  min-width: 0;
  min-height: 34px;
  align-items: center;
}

.request-target-url-value {
  overflow-wrap: anywhere;
  padding: 0 8px;
  color: var(--foreground);
  font-size: 12px;
  font-weight: 500;
}

.request-target-url :deep(.request-url-editor) {
  flex: 1 1 0;
  width: 100%;
  min-width: 0;
  min-height: 34px;
  border: 0;
  background: transparent;
  font-family: var(--font-mono);
  font-size: 12px;
}

.request-target-url :deep(.request-url-editor:focus-visible) {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}

.request-target-send {
  min-width: max-content;
  padding-inline-start: 8px;
}
.request-target-extra-actions {
  display: flex;
  gap: 8px;
  padding-inline-start: 8px;
}

.request-target-send :deep(button) {
  min-height: 34px;
  min-width: 78px;
}

.request-target-error,
.request-target-unsafe {
  margin-block: 8px 12px;
  border: 1px solid color-mix(in srgb, var(--destructive) 30%, transparent);
  border-radius: 5px;
  padding: 8px 10px;
  color: var(--destructive);
  background: color-mix(in srgb, var(--destructive) 5%, transparent);
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.request-target-public-hint {
  margin-block: 12px;
  border: 1px solid color-mix(in srgb, var(--primary) 20%, transparent);
  border-radius: 5px;
  padding: 8px 10px;
  color: var(--muted-foreground);
  background: color-mix(in srgb, var(--primary) 5%, transparent);
  font-size: 12px;
  line-height: 1.5;
}

@container openapi-request-setup (max-width: 42rem) {
  .openapi-request-target {
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px;
  }

  .request-target-send {
    grid-row: 2;
    grid-column: 1;
    padding-inline-start: 0;
  }

  .request-target-extra-actions {
    grid-row: 2;
    grid-column: 2;
    justify-content: flex-end;
  }

  .request-target-send :deep(button) {
    width: 100%;
    min-height: 34px;
  }

  .request-target-method-value,
  .request-target-url-value {
    min-height: 34px;
  }
}

@container openapi-request-setup (max-width: 20rem) {
  .openapi-request-target {
    grid-template-columns: minmax(0, 1fr);
  }

  .request-target-method,
  .request-target-url,
  .request-target-send {
    grid-column: 1;
    grid-row: auto;
  }
}
</style>
