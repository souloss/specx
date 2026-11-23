<script setup lang="ts">
import OpenApiDynamicValuePicker from "../variables/OpenApiDynamicValuePicker.vue";
import OpenApiVariableInput from "../variables/OpenApiVariableInput.vue";
import { mergeOpenApiSchema } from "@/lib/specifications/openapi/contract/openapi";
import AppSelect from "@/components/ui/AppSelect.vue";
import { computed, ref } from "vue";
import { Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    schema?: OpenApiObject;
    document?: OpenApiObject | null;
    depth?: number;
    name: string;
    label?: string;
    placeholder?: string;
    disabled?: boolean;
  }>(),
  { modelValue: "", schema: () => ({}), placeholder: "", disabled: false },
);
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const t = translate;
const kind = computed(() => (props.schema.type === "array" ? "array" : props.schema.type === "object" || isRecord(props.schema.properties) ? "object" : "scalar"));
const itemSchema = computed(() => (isRecord(props.schema.items) ? props.schema.items : {}));
const objectProperties = computed(() => (isRecord(props.schema.properties) ? props.schema.properties : {}));
const canAddArrayItem = computed(() => typeof props.schema.maxItems !== "number" || arrayItems.value.length < props.schema.maxItems);
const canAddObjectProperty = computed(() => {
  const current = parseObject();
  const hasUnusedDefinedProperty = Object.keys(objectProperties.value).some((name) => !Object.hasOwn(current, name));
  const allowsAdditionalProperties = props.schema.additionalProperties !== false;
  const belowMaxProperties = typeof props.schema.maxProperties !== "number" || Object.keys(current).length < props.schema.maxProperties;
  return belowMaxProperties && (hasUnusedDefinedProperty || allowsAdditionalProperties);
});

function parseArray(): unknown[] {
  if (!props.modelValue.trim()) return [];
  try {
    const value: unknown = JSON.parse(props.modelValue);
    if (Array.isArray(value)) return value;
  } catch {
    return props.modelValue.split(",").map((value) => value.trim());
  }
  return [];
}

function parseObject(): Record<string, unknown> {
  if (!props.modelValue.trim()) return {};
  try {
    const value: unknown = JSON.parse(props.modelValue);
    return isRecord(value) ? value : {};
  } catch {
    return {};
  }
}

const arrayItems = computed(parseArray);
const objectEntries = computed(() => Object.entries({ ...Object.fromEntries(Object.keys(objectProperties.value).map((key) => [key, undefined])), ...parseObject() }));
const rawMode = ref(/^\{\{[\s\S]*\}\}$/.test(props.modelValue.trim()));
const rawError = computed(() => {
  if (!rawMode.value || !props.modelValue.trim() || /^\{\{[\s\S]*\}\}$/.test(props.modelValue.trim())) return false;
  try {
    const value = JSON.parse(props.modelValue);
    return kind.value === "array" ? !Array.isArray(value) : !isRecord(value);
  } catch {
    return true;
  }
});

function valueSchema(name?: string): OpenApiObject {
  if (name && isRecord(objectProperties.value[name])) return mergeOpenApiSchema(props.document ?? null, objectProperties.value[name]) ?? (objectProperties.value[name] as OpenApiObject);
  if (isRecord(props.schema.additionalProperties)) return props.schema.additionalProperties as OpenApiObject;
  return itemSchema.value;
}

function isRequiredProperty(name: string): boolean {
  return Array.isArray(props.schema.required) && props.schema.required.includes(name);
}

function inputValue(value: unknown): string {
  if (value === undefined || value === null) return "";
  return typeof value === "string" ? value : typeof value === "object" ? JSON.stringify(value) : String(value);
}

function parseTypedValue(value: string, schema: OpenApiObject): unknown {
  if (schema.type === "integer" || schema.type === "number") {
    const parsed = Number(value);
    return value.trim() && Number.isFinite(parsed) && (schema.type !== "integer" || Number.isInteger(parsed)) ? parsed : value;
  }
  if (schema.type === "boolean") return value === "true" ? true : value === "false" ? false : value;
  if (schema.type === "object" || schema.type === "array") {
    try {
      const parsed: unknown = JSON.parse(value);
      if (schema.type === "array" ? Array.isArray(parsed) : isRecord(parsed)) return parsed;
    } catch {
      return value;
    }
  }
  return value;
}

function updateArrayItem(index: number, value: string) {
  const next = [...arrayItems.value];
  next[index] = parseTypedValue(value, itemSchema.value);
  emit("update:modelValue", JSON.stringify(next));
}

function addArrayItem() {
  emit("update:modelValue", JSON.stringify([...arrayItems.value, itemSchema.value.default ?? ""]));
}

function removeArrayItem(index: number) {
  if (arrayItems.value.length <= (typeof props.schema.minItems === "number" ? props.schema.minItems : 0)) return;
  emit("update:modelValue", JSON.stringify(arrayItems.value.filter((_, itemIndex) => itemIndex !== index)));
}

function updateObjectValue(name: string, value: string) {
  const next = { ...parseObject(), [name]: parseTypedValue(value, valueSchema(name)) };
  emit("update:modelValue", JSON.stringify(next));
}

function renameObjectProperty(oldName: string, input: HTMLInputElement) {
  const name = input.value.trim();
  const current = parseObject();
  if (!name || name === oldName || Object.hasOwn(current, name)) {
    input.value = oldName;
    return;
  }
  const next: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(current)) next[key === oldName ? name : key] = value;
  emit("update:modelValue", JSON.stringify(next));
}

function addObjectProperty() {
  if (!canAddObjectProperty.value) return;
  const current = parseObject();
  const candidate = Object.keys(objectProperties.value).find((name) => !Object.hasOwn(current, name));
  let index = 1;
  let name = candidate ?? `property${index}`;
  while (Object.hasOwn(current, name)) {
    index += 1;
    name = `property${index}`;
  }
  emit("update:modelValue", JSON.stringify({ ...current, [name]: valueSchema(name).default ?? "" }));
}

function removeObjectProperty(name: string) {
  if (isRequiredProperty(name) || objectEntries.value.length <= (typeof props.schema.minProperties === "number" ? props.schema.minProperties : 0)) return;
  const next = parseObject();
  delete next[name];
  emit("update:modelValue", JSON.stringify(next));
}
</script>

<template>
  <OpenApiVariableInput v-if="kind === 'scalar' && (schema.type !== 'boolean' || modelValue.includes('{{'))" :model-value="modelValue" :aria-label="label || name" :placeholder="placeholder" :disabled="disabled" :name="name" @update:model-value="emit('update:modelValue', $event)" />
  <div v-else-if="kind === 'scalar'" class="boolean-parameter-value">
    <AppSelect size="form" class="field-control" :value="modelValue" :disabled="disabled" :name="name" :aria-label="label || name" @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value)">
      <option value="">{{ placeholder || t("specification.openapiParameterValue") }}</option>
      <option value="true">true</option>
      <option value="false">false</option> </AppSelect
    ><OpenApiDynamicValuePicker :disabled="disabled" @insert="emit('update:modelValue', $event)" />
  </div>
  <div v-else class="openapi-parameter-structured" role="group" :aria-label="label || name" :data-parameter-name="name">
    <div class="structured-editor-toolbar">
      <button type="button" :aria-pressed="!rawMode" @click="rawMode = false">{{ t("specification.openapiDynamic.fields") }}</button><button type="button" :aria-pressed="rawMode" @click="rawMode = true">{{ t("specification.openapiDynamic.json") }}</button>
    </div>
    <template v-if="rawMode">
      <OpenApiVariableInput class="structured-json-input" multiline :name="name" :model-value="modelValue" :aria-label="label || name" :aria-invalid="rawError" :disabled="disabled" @update:model-value="emit('update:modelValue', $event)" />
      <p v-if="rawError" role="alert" class="text-xs text-destructive">{{ t("specification.openapiDynamic.invalidJson") }}</p></template
    >
    <template v-else-if="kind === 'array'">
      <div v-for="(item, index) in arrayItems" :key="index" class="openapi-parameter-structured-row array">
        <OpenApiParameterValueEditor
          v-if="(depth ?? 0) < 8"
          :model-value="inputValue(item)"
          :schema="mergeOpenApiSchema(document ?? null, itemSchema) ?? itemSchema"
          :document="document"
          :depth="(depth ?? 0) + 1"
          :name="`${name}-item-${index}`"
          :disabled="disabled"
          :placeholder="placeholder"
          @update:model-value="updateArrayItem(index, $event)"
        /><OpenApiVariableInput v-else :model-value="inputValue(item)" :name="`${name}-item-${index}`" :aria-label="t('specification.openapiDynamic.recursive')" @update:model-value="updateArrayItem(index, $event)" />
        <button type="button" class="icon-button" :aria-label="t('specification.openapiParameterRemoveValue', { index: index + 1 })" :disabled="disabled || arrayItems.length <= (typeof schema.minItems === 'number' ? schema.minItems : 0)" @click="removeArrayItem(index)">
          <Trash2 class="h-3.5 w-3.5" />
        </button>
      </div>
    </template>
    <template v-else>
      <datalist :id="`${name}-properties`"><option v-for="property in Object.keys(objectProperties)" :key="property" :value="property" /></datalist>
      <div v-for="[key, value] in objectEntries" :key="key" class="openapi-parameter-structured-row object">
        <input
          class="field-control field-control-compact font-mono"
          :value="key"
          :list="`${name}-properties`"
          :readonly="Object.hasOwn(objectProperties, key)"
          :aria-label="t('specification.openapiParameterPropertyName')"
          :disabled="disabled"
          autocomplete="off"
          :name="`${name}-key-${key}`"
          @change="renameObjectProperty(key, $event.target as HTMLInputElement)"
        />
        <div class="structured-property-value" :class="{ nested: valueSchema(key).type === 'object' || valueSchema(key).type === 'array' || Boolean(valueSchema(key).properties) }">
          <OpenApiParameterValueEditor
            v-if="(depth ?? 0) < 8"
            :model-value="inputValue(value)"
            :schema="valueSchema(key)"
            :document="document"
            :depth="(depth ?? 0) + 1"
            :name="`${name}-value-${key}`"
            :label="key"
            :disabled="disabled"
            :placeholder="placeholder"
            @update:model-value="updateObjectValue(key, $event)"
          /><OpenApiVariableInput v-else :model-value="inputValue(value)" :name="`${name}-value-${key}`" :aria-label="t('specification.openapiDynamic.recursive')" @update:model-value="updateObjectValue(key, $event)" />
        </div>
        <span class="structured-property-type">{{ valueSchema(key).type ?? "string" }}</span
        ><span class="structured-property-description">{{ valueSchema(key).description ?? "" }}</span>
        <button
          type="button"
          class="icon-button"
          :aria-label="t('specification.openapiParameterRemoveProperty', { name: key })"
          :disabled="disabled || (Array.isArray(schema.required) && schema.required.includes(key)) || objectEntries.length <= (typeof schema.minProperties === 'number' ? schema.minProperties : 0)"
          @click="removeObjectProperty(key)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </button>
      </div>
    </template>
    <Button v-if="!rawMode && (kind === 'array' ? canAddArrayItem : canAddObjectProperty)" size="xs" variant="ghost" class="openapi-parameter-add-value" :disabled="disabled" @click="kind === 'array' ? addArrayItem() : addObjectProperty()"
      ><Plus class="h-3.5 w-3.5" />{{ kind === "array" ? t("specification.openapiParameterAddItem") : t("specification.openapiParameterAddProperty") }}</Button
    >
  </div>
</template>

<style scoped>
.boolean-parameter-value {
  display: flex;
  align-items: center;
  min-width: 0;
}
.boolean-parameter-value .app-select {
  flex: 1;
  min-width: 0;
}

.structured-editor-toolbar {
  display: flex;
  gap: 4px;
  padding-bottom: 4px;
}
.structured-editor-toolbar button {
  font-size: 11px;
  padding: 3px 8px;
  min-height: 24px;
  border-radius: 4px;
  color: var(--muted-foreground);
}
.structured-editor-toolbar button[aria-pressed="true"] {
  background: var(--accent);
  color: var(--primary);
}
.openapi-parameter-structured :deep(.structured-json-input) {
  min-height: 100px;
  width: 100%;
  resize: vertical;
  font-family: var(--font-mono);
  font-size: 12px;
}
.structured-property-value {
  min-width: 0;
}
.structured-property-type,
.structured-property-description {
  font-size: 11px;
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
  min-width: 0;
}
.openapi-parameter-structured-row.object {
  grid-template-columns: minmax(0, 24%) minmax(0, 28%) minmax(0, 14%) minmax(0, 1fr) 24px;
  align-items: start;
  min-height: 30px;
}
.openapi-parameter-structured-row.object > :first-child {
  grid-column: 1;
  grid-row: 1;
}
.structured-property-type {
  grid-column: 3;
  grid-row: 1;
}
.structured-property-description {
  grid-column: 4;
  grid-row: 1;
}
.openapi-parameter-structured-row.object > button {
  grid-column: 5;
  grid-row: 1;
}
.structured-property-value.nested {
  grid-column: 1 / -1;
  grid-row: 2;
  padding-left: 16px;
  border-left: 1px solid var(--border);
}
.openapi-parameter-structured-row {
  min-width: 0;
}
.openapi-parameter-structured {
  min-width: 0;
  width: 100%;
}
.openapi-parameter-structured :deep(.app-select.field-control) {
  border: 0;
  background: transparent;
  box-shadow: none;
  min-height: 28px;
  padding: 3px 7px;
  font-size: 12px;
}
@container parameter-value (max-width: 20rem) {
  .openapi-parameter-structured-row.object {
    grid-template-columns: minmax(0, 1fr) 50px 24px;
  }
  .openapi-parameter-structured-row.object > :first-child {
    grid-column: 1;
    grid-row: 1;
  }
  .structured-property-type {
    grid-column: 2;
  }
  .structured-property-description {
    display: none;
  }
  .openapi-parameter-structured-row.object > button {
    grid-column: 3;
  }
  .structured-property-value {
    grid-column: 1 / -1;
    grid-row: 2;
  }
  .structured-property-value.nested {
    padding-left: 10px;
  }
}
</style>
