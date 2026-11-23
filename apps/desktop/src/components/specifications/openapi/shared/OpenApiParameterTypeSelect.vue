<script setup lang="ts">
import { computed, defineAsyncComponent, ref } from "vue";
import { Settings2 } from "@lucide/vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import AppPopover from "@/components/ui/AppPopover.vue";
import { Dialog, DialogScrollContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { getOpenApiModels } from "@/lib/specifications/openapi/contract/openapiModels";
import { debugFieldSchema } from "@/lib/specifications/openapi/workbench/request/openapiDebugFields";
import type { OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import type { OpenApiParameterConfiguration } from "@/lib/specifications/openapi/workbench/openapiWorkbench";
import { translate as t } from "@/i18n";
const model = defineModel<string>({ default: "string" });
const props = defineProps<{ name?: string; primitiveOnly?: boolean; file?: boolean; document?: OpenApiObject | null; schema?: OpenApiObject; location?: string; style?: string; explode?: boolean; allowReserved?: boolean; disabled?: boolean; allowLocationChange?: boolean }>();
const emit = defineEmits<{ configuration: [value: OpenApiParameterConfiguration] }>();
const types = computed(() => ["string", "integer", "boolean", "number", ...(props.primitiveOnly ? [] : ["array", "object", ...(props.file ? ["file"] : [])])]);
const references = computed(() => (props.primitiveOnly ? [] : getOpenApiModels(props.document ?? null)));
const OpenApiSchemaDesignEditor = defineAsyncComponent(() => import("../design/OpenApiSchemaDesignEditor.vue"));
const OpenApiSchemaModelPicker = defineAsyncComponent(() => import("./OpenApiSchemaModelPicker.vue"));
const picker = ref(false),
  settings = ref(false),
  dialog = ref(false),
  draft = ref<OpenApiObject>({});
const structured = computed(() => !props.primitiveOnly && (model.value === "object" || model.value === "array" || model.value.startsWith("#/")));
const hasAdvancedSettings = computed(() => Boolean(props.location) || structured.value);
const resolved = computed(() => props.schema ?? debugFieldSchema(model.value, props.document));
const styles = computed(() => (props.location === "path" ? ["simple", "label", "matrix"] : props.location === "query" ? ["form", "deepObject", "spaceDelimited", "pipeDelimited"] : props.location === "header" ? ["simple"] : ["form"]));
function choose(value: string) {
  if (value === "__reference__") {
    picker.value = true;
    return;
  }
  model.value = value;
}
function edit() {
  draft.value = structuredClone(resolved.value);
  settings.value = false;
  dialog.value = true;
}
function apply() {
  emit("configuration", { schema: draft.value });
  dialog.value = false;
}
</script>
<template>
  <div class="debug-parameter-type-control">
    <AppSelect
      :model-value="model"
      @update:model-value="choose"
      :name="name"
      size="compact"
      variant="quiet"
      class="debug-type-select"
      :class="{ 'is-reference': model.startsWith('#/') }"
      content-class="debug-type-menu openapi-workbench-surface"
      :aria-label="t('specification.openapiParameterType')"
      :disabled="disabled"
    >
      <option v-if="!types.includes(model) && !references.some((item) => item.reference === model)" :value="model">{{ model }}</option>
      <option v-for="type in types" :key="type" :value="type">{{ type }}</option>
      <option v-if="!primitiveOnly" value="__reference__">{{ t("specification.openapiDynamic.reference") }}</option>
      <optgroup v-if="references.length" :label="t('specification.openapiReferenceType')">
        <option v-for="item in references" :key="item.reference" :value="item.reference">{{ item.name }}</option>
      </optgroup>
    </AppSelect>
    <AppPopover v-if="hasAdvancedSettings" v-model:open="settings" align="end"
      ><template #trigger
        ><button type="button" class="icon-button parameter-settings-trigger" :aria-label="t('specification.openapiDynamic.schemaSettings')" :title="t('specification.openapiDynamic.schemaSettings')"><Settings2 class="size-3" aria-hidden="true" /></button
      ></template>
      <div class="parameter-settings-panel">
        <strong>{{ t("specification.openapiDynamic.schemaSettings") }}</strong
        ><label v-if="allowLocationChange" class="parameter-location-field"
          >{{ t("specification.openapiLocation")
          }}<AppSelect :model-value="location" size="compact" name="parameter-location" @update:model-value="emit('configuration', { location: $event })"
            ><option value="query">query</option>
            <option value="header">header</option>
            <option value="cookie">cookie</option></AppSelect
          ></label
        ><button v-if="structured" type="button" class="parameter-schema-edit" @click="edit">{{ t("specification.openapiDynamic.schemaTitle") }}</button
        ><template v-if="location"
          ><div class="parameter-serialization-fields">
            <label
              >style<AppSelect :model-value="style ?? ''" size="compact" name="parameter-style" @update:model-value="emit('configuration', { style: $event })"
                ><option value="">{{ t("specification.openapiDesignSerializationDefault") }}</option>
                <option v-for="item in styles" :key="item" :value="item">{{ item }}</option></AppSelect
              ></label
            ><label
              >explode<AppSelect :model-value="explode === undefined ? '' : explode" size="compact" name="parameter-explode" @update:model-value="emit('configuration', { explode: $event === '' ? undefined : Boolean($event) })"
                ><option value="">{{ t("specification.openapiDesignSerializationDefault") }}</option>
                <option :value="true">true</option>
                <option :value="false">false</option></AppSelect
              ></label
            >
          </div>
          <label v-if="location === 'query'" class="parameter-allow-reserved"><input type="checkbox" :checked="allowReserved" @change="emit('configuration', { allowReserved: ($event.target as HTMLInputElement).checked })" />{{ t("specification.openapiDesignAllowReserved") }}</label>
          <p>{{ t("specification.openapiDynamic.styleHint") }}</p></template
        >
      </div></AppPopover
    >
  </div>
  <OpenApiSchemaModelPicker v-if="picker" v-model:open="picker" :document="document ?? null" :label="name ?? ''" :current-reference="model" @select="choose" />
  <Dialog v-model:open="dialog"
    ><DialogScrollContent class="parameter-schema-dialog sm:max-w-3xl"
      ><DialogHeader
        ><DialogTitle>{{ t("specification.openapiDynamic.schemaTitle") }}</DialogTitle
        ><DialogDescription>{{ t("specification.openapiDynamic.schemaHint") }}</DialogDescription></DialogHeader
      ><OpenApiSchemaDesignEditor v-if="dialog" v-model="draft" :label="name ?? ''" :document="document" /><DialogFooter
        ><Button variant="outline" @click="dialog = false">{{ t("common.cancel") }}</Button
        ><Button @click="apply">{{ t("common.save") }}</Button></DialogFooter
      ></DialogScrollContent
    ></Dialog
  >
</template>
<style scoped>
.debug-parameter-type-control {
  display: flex;
  align-items: center;
  min-width: 0;
  width: 100%;
}
.debug-type-select {
  flex: 1;
  min-width: 0;
}
.debug-type-select.is-reference {
  color: var(--primary);
}
.parameter-settings-trigger {
  flex-shrink: 0;
  width: 22px;
  min-height: 24px;
  color: var(--muted-foreground);
}
.parameter-settings-panel {
  width: 300px;
  max-width: calc(100vw - 50px);
  font-size: 12px;
  display: grid;
  gap: 10px;
}
.parameter-schema-edit {
  text-align: start;
  color: var(--primary);
  padding: 5px 0;
}
.parameter-serialization-fields {
  display: flex;
  gap: 12px;
}
.parameter-serialization-fields label {
  display: grid;
  gap: 5px;
  flex: 1;
}
.parameter-location-field {
  display: grid;
  gap: 5px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.parameter-allow-reserved {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.parameter-settings-panel p {
  color: var(--muted-foreground);
}
</style>
