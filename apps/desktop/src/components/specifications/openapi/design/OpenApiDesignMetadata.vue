<script setup lang="ts">
import { OPENAPI_METHODS } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiOperationAddressError } from "@/lib/specifications/openapi/design/openapiOperationEditing";
import { translate as t } from "@/i18n";
import AppSelect from "@/components/ui/AppSelect.vue";
import { Check, Copy } from "@lucide/vue";
import { useClipboardFeedback } from "@/composables/useClipboardFeedback";

const props = defineProps<{ editable: boolean; addressError: OpenApiOperationAddressError | null }>();
const method = defineModel<string>("method", { required: true });
const path = defineModel<string>("path", { required: true });
const summary = defineModel<string>("summary", { required: true });
const operationId = defineModel<string>("operationId", { required: true });
const tags = defineModel<string>("tags", { required: true });
const description = defineModel<string>("description", { required: true });
const { copy, copied, failed } = useClipboardFeedback();

function copyOperationPath() {
  const value = path.value.trim();
  if (value) void copy(value);
}
</script>

<template>
  <section class="openapi-design-metadata openapi-design-wide">
    <div class="openapi-design-operation-toolbar">
      <div class="openapi-design-address-fields">
        <label class="openapi-design-method-control">
          <span class="sr-only">{{ t("specification.openapiOperationMethod") }}</span>
          <AppSelect v-model="method" size="form" name="openapi-design-method" class="field-control font-mono" :disabled="!props.editable">
            <option v-for="item in OPENAPI_METHODS" :key="item" :value="item">{{ item.toUpperCase() }}</option>
          </AppSelect>
        </label>
        <button
          type="button"
          class="icon-button openapi-design-copy-path"
          :class="{ copied, failed }"
          :aria-label="t(copied ? 'specification.openapiOperationPathCopied' : 'specification.openapiOperationPathCopy')"
          :title="t(copied ? 'specification.openapiOperationPathCopied' : 'specification.openapiOperationPathCopy')"
          :disabled="!path.trim()"
          @click="copyOperationPath"
        >
          <Check v-if="copied" class="size-3.5" aria-hidden="true" />
          <Copy v-else class="size-3.5" aria-hidden="true" />
        </button>
        <label class="openapi-design-path-control">
          <span class="sr-only">{{ t("specification.openapiOperationPath") }}</span>
          <input
            v-model="path"
            name="openapi-design-path"
            class="field-control font-mono"
            autocomplete="off"
            :spellcheck="false"
            :disabled="!props.editable"
            :aria-invalid="Boolean(props.addressError)"
            :aria-describedby="props.addressError ? 'openapi-design-address-error' : 'openapi-design-address-hint'"
          />
        </label>
      </div>
      <span class="sr-only" role="status" aria-live="polite">{{ failed ? t("specification.openapiOperationPathCopyFailed") : copied ? t("specification.openapiOperationPathCopied") : "" }}</span>
      <slot name="actions" />
    </div>
    <p v-if="props.addressError" id="openapi-design-address-error" class="openapi-design-message error" role="alert">{{ t(`specification.openapiOperationAddressError.${props.addressError}`) }}</p>
    <p id="openapi-design-address-hint" class="sr-only">{{ t("specification.openapiOperationAddressHint") }}</p>
    <div class="openapi-design-shortcuts" :aria-label="t('specification.openapiDesignOperationShortcuts')">
      <slot name="shortcuts" />
    </div>
    <details class="openapi-design-metadata-details">
      <summary>
        <span>{{ t("specification.openapiDesignMetadataTitle") }}</span>
        <span v-if="summary" class="openapi-design-metadata-summary">{{ summary }}</span>
      </summary>
      <div class="openapi-design-metadata-fields">
        <label class="openapi-design-field">
          <span>{{ t("specification.openapiDesignSummary") }}</span>
          <input v-model="summary" name="openapi-design-summary" class="field-control" autocomplete="off" :disabled="!props.editable" />
        </label>
        <label class="openapi-design-field">
          <span>{{ t("specification.openapiDesignOperationId") }}</span>
          <input v-model="operationId" name="openapi-design-operation-id" class="field-control font-mono" autocomplete="off" :spellcheck="false" :disabled="!props.editable" />
        </label>
        <label class="openapi-design-field">
          <span>{{ t("specification.openapiDesignTags") }}</span>
          <input v-model="tags" name="openapi-design-tags" class="field-control" autocomplete="off" :disabled="!props.editable" :placeholder="t('specification.openapiDesignTagsPlaceholder')" />
        </label>
        <label class="openapi-design-field openapi-design-description-field">
          <span>{{ t("specification.openapiDescription") }}</span>
          <textarea v-model="description" name="openapi-design-description" class="field-control openapi-design-textarea" autocomplete="off" :disabled="!props.editable" rows="3" />
        </label>
      </div>
    </details>
  </section>
</template>
