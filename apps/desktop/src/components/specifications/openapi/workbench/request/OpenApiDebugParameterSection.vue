<script setup lang="ts">
import { computed, ref } from "vue";
import OpenApiBulkEditDialog from "./OpenApiBulkEditDialog.vue";
import { Eye, EyeOff } from "@lucide/vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import OpenApiDebugParameterTable, { type DebugParameter } from "./OpenApiDebugParameterTable.vue";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiParameterConfiguration, WorkbenchRow } from "@/lib/specifications/openapi/workbench/openapiWorkbench";
import { translate as t } from "@/i18n";

const props = withDefaults(
  defineProps<{
    section: "path" | "query" | "header" | "cookie";
    title: string;
    parameters: DebugParameter[];
    editable?: boolean;
    showComputed?: boolean;
    computedRows?: DebugParameter[];
    document?: OpenApiObject | null;
  }>(),
  { editable: true, showComputed: undefined },
);
const bulkOpen = ref(false);
const rows = defineModel<WorkbenchRow[]>("rows", { default: () => [] });
const visibleParameters = computed(() => [...props.parameters, ...(props.showComputed ? (props.computedRows ?? []) : [])]);
const emit = defineEmits<{
  value: [key: string, value: string];
  enabled: [key: string, value: boolean];
  apply: [rows: WorkbenchRow[]];
  configuration: [key: string, value: OpenApiParameterConfiguration];
  metadata: [key: string, field: "type" | "description" | "name", value: string];
  "update:showComputed": [value: boolean];
}>();
</script>

<template>
  <section class="debug-field-group">
    <div class="debug-field-group-heading">
      <div class="debug-field-title">
        <h5>{{ title }}</h5>
        <AppTooltip v-if="showComputed !== undefined" :text="t(showComputed ? 'specification.openapiHideComputedHeaders' : 'specification.openapiShowComputedHeaders')">
          <button
            type="button"
            class="debug-generated-toggle icon-button"
            :aria-expanded="showComputed"
            :aria-controls="`openapi-${section}-table`"
            :aria-label="t(showComputed ? 'specification.openapiHideComputedHeaders' : 'specification.openapiShowComputedHeaders')"
            @click="emit('update:showComputed', !showComputed)"
          >
            <Eye v-if="showComputed" class="size-3.5" /><EyeOff v-else class="size-3.5" />
            <span>{{ computedRows?.length ?? 0 }}</span>
          </button>
        </AppTooltip>
      </div>
      <div v-if="editable" class="debug-field-heading-actions">
        <button :id="`openapi-${section}-mode-bulk`" class="debug-bulk-trigger" type="button" @click="bulkOpen = true">{{ t("specification.openapiBulkEdit") }}</button>
      </div>
    </div>
    <OpenApiDebugParameterTable
      :id="`openapi-${section}-table`"
      v-model:rows="rows"
      :section="section"
      :parameters="visibleParameters"
      :editable="editable"
      :document="document"
      @value="(key, value) => emit('value', key, value)"
      @enabled="(key, value) => emit('enabled', key, value)"
      @configuration="(key, value) => emit('configuration', key, value)"
      @metadata="(key, field, value) => emit('metadata', key, field, value)"
    />
    <OpenApiBulkEditDialog v-if="editable" v-model:open="bulkOpen" :rows="[...props.parameters.map((p) => ({ name: p.name, value: p.value, enabled: p.enabled, type: p.type, description: p.description })), ...rows]" @apply="emit('apply', $event)" />
  </section>
</template>
