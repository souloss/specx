<script setup lang="ts">
import { computed } from "vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate as t, type MessageKey } from "@/i18n";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { constraintValueText, isOpenApi31, parseSchemaFieldValue, schemaAllowsNull } from "@/lib/specifications/openapi/design/openapiSchemaFields";
import OpenApiSchemaEnum from "./OpenApiSchemaEnum.vue";

const schema = defineModel<OpenApiObject>({ required: true });
const required = defineModel<boolean | undefined>("required", { default: undefined });
const props = withDefaults(defineProps<{ type: string; document?: OpenApiObject | null; reference?: boolean; requiredLocked?: boolean; fieldName: string; deprecated?: boolean }>(), { deprecated: undefined });
const emit = defineEmits<{ "update:deprecated": [boolean] }>();
const modern = computed(() => isOpenApi31(props.document));
const nullable = computed(() => schemaAllowsNull(schema.value));
const numeric = computed(() => props.type === "integer" || props.type === "number");
const behavior = computed(() => (schema.value.readOnly === true ? "read" : schema.value.writeOnly === true ? "write" : ""));
const counts: Record<string, [string, MessageKey][]> = {
  string: [
    ["minLength", "specification.openapiSchemaMinLength"],
    ["maxLength", "specification.openapiSchemaMaxLength"],
  ],
  array: [
    ["minItems", "specification.openapiSchemaMinItems"],
    ["maxItems", "specification.openapiSchemaMaxItems"],
  ],
  object: [
    ["minProperties", "specification.openapiSchemaMinProperties"],
    ["maxProperties", "specification.openapiSchemaMaxProperties"],
  ],
};
const bounds = [
  ["minimum", "exclusiveMinimum", "specification.openapiSchemaMinimum", "specification.openapiSchemaExclusiveMinimum"],
  ["maximum", "exclusiveMaximum", "specification.openapiSchemaMaximum", "specification.openapiSchemaExclusiveMaximum"],
] as const;
const isDeprecated = computed(() => props.deprecated ?? schema.value.deprecated === true);
const valueText = (value: unknown) => constraintValueText(value, nullable.value);
const checked = (event: Event) => (event.target as HTMLInputElement).checked;

function update(values: OpenApiObject) {
  const next = { ...schema.value };
  for (const [key, value] of Object.entries(values)) {
    if (value === undefined) delete next[key];
    else next[key] = value;
  }
  schema.value = next;
}

function setNullable(enabled: boolean) {
  if (!props.type) return;
  if (modern.value) update({ type: enabled ? [props.type, "null"] : props.type, nullable: undefined });
  else update({ type: typeof schema.value.type === "string" ? schema.value.type : props.type, nullable: enabled || undefined });
}

function setDeprecated(value: boolean) {
  if (props.deprecated === undefined) update({ deprecated: value || undefined });
  else emit("update:deprecated", value);
}

function numericValue(event: Event, integer: boolean, minimum: number, exclusive: boolean, previous: unknown): number | undefined {
  const input = event.target as HTMLInputElement;
  if (!input.validity.badInput && !input.value.trim()) return undefined;
  const value = input.valueAsNumber;
  if (Number.isFinite(value) && (!integer || Number.isInteger(value)) && (exclusive ? value > minimum : value >= minimum)) return value;
  input.value = typeof previous === "number" ? String(previous) : "";
  return typeof previous === "number" ? previous : undefined;
}

function setNumber(key: string, event: Event, count = false) {
  update({ [key]: numericValue(event, count, key === "multipleOf" || count ? 0 : -Infinity, key === "multipleOf", schema.value[key]) });
}

function boundValue(key: string, exclusive: string) {
  return typeof schema.value[exclusive] === "number" ? schema.value[exclusive] : schema.value[key];
}

function isExclusive(exclusive: string) {
  return typeof schema.value[exclusive] === "number" || schema.value[exclusive] === true;
}

function setBound(key: string, exclusive: string, value: unknown, exclusiveEnabled: boolean) {
  if (modern.value) update({ [key]: exclusiveEnabled ? undefined : value, [exclusive]: exclusiveEnabled ? value : undefined });
  else update({ [key]: value, [exclusive]: value !== undefined && exclusiveEnabled ? true : undefined });
}

function setValue(key: "default" | "const" | "example", event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.value) update({ [key]: undefined });
  else {
    const parsed = parseSchemaFieldValue(input.value, props.type, nullable.value);
    if (parsed) update({ [key]: parsed.value });
    else input.value = valueText(schema.value[key]);
  }
}

function setExamples(event: Event) {
  const input = event.target as HTMLTextAreaElement;
  const values = input.value
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => parseSchemaFieldValue(line, props.type, nullable.value));
  if (values.every(Boolean)) update({ examples: values.length ? values.map((entry) => entry?.value) : undefined });
  else input.value = examplesText.value;
}

const examplesText = computed(() => (Array.isArray(schema.value.examples) ? schema.value.examples.map(valueText).join("\n") : ""));
</script>

<template>
  <div class="schema-constraint-form">
    <div class="schema-constraint-flags">
      <label v-if="required !== undefined" class="schema-constraint-toggle"> <input v-model="required" type="checkbox" role="switch" :disabled="requiredLocked" />{{ t("specification.openapiRequired") }} </label>
      <label v-if="type && type !== 'null' && !reference" class="schema-constraint-toggle"> <input type="checkbox" role="switch" :name="`schema-nullable-${fieldName}`" :checked="nullable" @change="setNullable(checked($event))" />{{ t("specification.openapiSchemaNullable") }} </label>
      <label class="schema-constraint-toggle"> <input type="checkbox" role="switch" :checked="isDeprecated" :name="`schema-deprecated-${fieldName}`" @change="setDeprecated(checked($event))" />{{ t("specification.openapiParameterDeprecated") }} </label>
    </div>
    <OpenApiSchemaEnum v-model="schema" :type="type" :nullable="nullable" />
    <div class="schema-constraint-grid">
      <label v-if="type === 'string' || numeric" class="schema-design-control">
        <span>{{ t("specification.openapiSchemaFormat") }}</span>
        <input :value="schema.format" class="field-control field-control-compact" :name="`schema-format-${fieldName}`" @change="update({ format: ($event.target as HTMLInputElement).value.trim() || undefined })" />
      </label>
      <label class="schema-design-control">
        <span>{{ t("specification.openapiSchemaBehavior") }}</span>
        <AppSelect :value="behavior" size="compact" :aria-label="t('specification.openapiSchemaBehavior')" @update:model-value="update({ readOnly: $event === 'read' || undefined, writeOnly: $event === 'write' || undefined })">
          <option value="">{{ t("specification.openapiSchemaReadWrite") }}</option>
          <option value="read">{{ t("specification.openapiSchemaReadOnly") }}</option>
          <option value="write">{{ t("specification.openapiSchemaWriteOnly") }}</option>
        </AppSelect>
      </label>
      <label v-for="[key, label] in counts[type] ?? []" :key="key" class="schema-design-control">
        <span>{{ t(label) }}</span>
        <input type="number" min="0" step="1" class="field-control field-control-compact" :value="schema[key]" :name="`schema-${key}-${fieldName}`" @change="setNumber(key, $event, true)" />
      </label>
      <template v-if="numeric">
        <div v-for="[key, exclusive, label, exclusiveLabel] in bounds" :key="key" class="grid gap-1">
          <label class="schema-design-control">
            <span>{{ t(label) }}</span>
            <input type="number" step="any" class="field-control field-control-compact" :value="boundValue(key, exclusive)" :name="`schema-${key}-${fieldName}`" @change="setBound(key, exclusive, numericValue($event, false, -Infinity, false, boundValue(key, exclusive)), isExclusive(exclusive))" />
          </label>
          <label class="schema-constraint-toggle">
            <input type="checkbox" :checked="isExclusive(exclusive)" :disabled="boundValue(key, exclusive) === undefined" :name="`schema-${exclusive}-${fieldName}`" @change="setBound(key, exclusive, boundValue(key, exclusive), checked($event))" />{{ t(exclusiveLabel) }}
          </label>
        </div>
        <label class="schema-design-control">
          <span>{{ t("specification.openapiSchemaMultipleOf") }}</span>
          <input type="number" step="any" class="field-control field-control-compact" :value="schema.multipleOf" @change="setNumber('multipleOf', $event)" />
        </label>
      </template>
      <label v-if="type === 'array'" class="schema-constraint-toggle"> <input type="checkbox" :checked="schema.uniqueItems === true" @change="update({ uniqueItems: checked($event) || undefined })" />{{ t("specification.openapiSchemaUniqueItems") }} </label>
      <label v-if="type === 'object'" class="schema-design-control">
        <span>{{ t("specification.openapiSchemaAdditionalProperties") }}</span>
        <AppSelect
          :value="isRecord(schema.additionalProperties) ? 'schema' : schema.additionalProperties === false ? 'forbidden' : 'allowed'"
          size="compact"
          :aria-label="t('specification.openapiSchemaAdditionalProperties')"
          @update:model-value="update({ additionalProperties: $event === 'schema' ? { type: 'string' } : $event === 'forbidden' ? false : undefined })"
        >
          <option value="allowed">{{ t("specification.openapiSchemaAdditionalAllowed") }}</option>
          <option value="forbidden">{{ t("specification.openapiSchemaAdditionalForbidden") }}</option>
          <option value="schema">{{ t("specification.openapiSchemaAdditionalTyped") }}</option>
        </AppSelect>
      </label>
      <label class="schema-design-control">
        <span>{{ t("specification.openapiSchemaDefault") }}</span>
        <input :value="valueText(schema.default)" class="field-control field-control-compact" :name="`schema-default-${fieldName}`" @change="setValue('default', $event)" />
      </label>
      <label v-if="type === 'string'" class="schema-design-control">
        <span>{{ t("specification.openapiSchemaPattern") }}</span>
        <input :value="schema.pattern" class="field-control field-control-compact font-mono" :name="`schema-pattern-${fieldName}`" @change="update({ pattern: ($event.target as HTMLInputElement).value || undefined })" />
      </label>
      <label v-if="modern" class="schema-design-control">
        <span>{{ t("specification.openapiSchemaConst") }}</span>
        <input :value="valueText(schema.const)" class="field-control field-control-compact" @change="setValue('const', $event)" />
      </label>
      <label v-if="!modern" class="schema-design-control">
        <span>{{ t("specification.openapiSchemaExample") }}</span>
        <input :value="valueText(schema.example)" class="field-control field-control-compact" @change="setValue('example', $event)" />
      </label>
      <label v-else class="schema-design-control col-span-full">
        <span>{{ t("specification.openapiSchemaExamples") }}</span>
        <textarea :value="examplesText" class="field-control" rows="2" :placeholder="t('specification.openapiSchemaExamplesPlaceholder')" @change="setExamples" />
      </label>
    </div>
  </div>
</template>

<style>
.schema-constraint-form {
  display: grid;
  gap: 12px;
}
.schema-constraint-flags,
.schema-constraint-toggle {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}
.schema-constraint-flags {
  gap: 8px 18px;
  padding-block-end: 10px;
  border-block-end: 1px solid var(--border);
}
.schema-constraint-toggle {
  width: fit-content;
  font-size: 12px;
}
.schema-constraint-toggle > input[role="switch"] {
  appearance: none;
  position: relative;
  width: 24px;
  height: 14px;
  border: 1px solid var(--muted-foreground);
  border-radius: 999px;
  background: var(--muted);
}
.schema-constraint-toggle > input[role="switch"]::before {
  content: "";
  position: absolute;
  inset-block-start: 2px;
  inset-inline-start: 2px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--muted-foreground);
}
.schema-constraint-toggle > input[role="switch"]:checked {
  border-color: var(--primary);
  background: var(--primary);
}
.schema-constraint-toggle > input[role="switch"]:checked::before {
  inset-inline-start: 12px;
  background: var(--primary-foreground);
}
.schema-constraint-toggle > input:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.schema-constraint-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  align-items: start;
  gap: 12px;
}
.schema-constraint-grid .app-select {
  width: 100%;
}
.schema-constraint-grid textarea.field-control {
  min-height: 3.5rem;
}
@media (max-width: 420px) {
  .schema-constraint-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
