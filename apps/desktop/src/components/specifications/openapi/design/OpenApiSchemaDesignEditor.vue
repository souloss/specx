<script setup lang="ts">
import AppSelect from "@/components/ui/AppSelect.vue";
import { computed, ref } from "vue";
import { Layers, Plus, Trash2, Unlink } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
import { isRecord, materializeOpenApiSchema, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { schemaFieldType, schemaModelReference } from "@/lib/specifications/openapi/design/openapiSchemaFields";
import OpenApiSchemaFieldType from "./OpenApiSchemaFieldType.vue";

const props = withDefaults(
  defineProps<{
    modelValue: OpenApiObject;
    label: string;
    editable?: boolean;
    depth?: number;
    document?: OpenApiObject | null;
    ancestorReferences?: string[];
    hideRootControls?: boolean;
    compact?: boolean;
  }>(),
  { editable: true, depth: 0, document: null, ancestorReferences: () => [] },
);
const emit = defineEmits<{ "update:modelValue": [schema: OpenApiObject] }>();
const t = translate;
const sourceReference = computed(() => (typeof props.modelValue.$ref === "string" ? props.modelValue.$ref : ""));
const schema = computed(() => {
  if (!sourceReference.value || props.ancestorReferences.includes(sourceReference.value)) return props.modelValue;
  return materializeOpenApiSchema(props.document, props.modelValue);
});
const schemaReference = computed(() => (typeof schema.value.$ref === "string" ? schema.value.$ref : ""));
const childReferences = computed(() => (sourceReference.value ? [...props.ancestorReferences, sourceReference.value] : props.ancestorReferences));
const hasUnsupportedComposition = computed(() => ["not", "if", "then", "else", "$dynamicRef"].some((key) => key in schema.value));
const compositionKeys = ["oneOf", "anyOf", "allOf"] as const;
type CompositionKey = (typeof compositionKeys)[number];
const compositions = computed(() => compositionKeys.flatMap((key) => (Array.isArray(schema.value[key]) ? [{ key, branches: schema.value[key] as unknown[] }] : [])));
const properties = computed(() => (isRecord(schema.value.properties) ? Object.entries(schema.value.properties) : []));
const required = computed(() => (Array.isArray(schema.value.required) ? schema.value.required.filter((item): item is string => typeof item === "string") : []));
const expandedProperties = ref<Set<string>>(new Set());
const valueType = computed(() => schemaFieldType(schema.value));
const canEdit = computed(() => props.editable && !schemaReference.value && !hasUnsupportedComposition.value);

function patch(patchValue: OpenApiObject) {
  if (canEdit.value) emit("update:modelValue", { ...schema.value, ...patchValue });
}

function detachModel() {
  if (props.editable && sourceReference.value && !schemaReference.value) emit("update:modelValue", JSON.parse(JSON.stringify(schema.value)) as OpenApiObject);
}

function setAdditionalProperties(mode: string) {
  patch({ additionalProperties: mode === "schema" ? { type: "string" } : mode === "allowed" });
}

function addComposition(key: string) {
  if (!canEdit.value || !compositionKeys.includes(key as CompositionKey) || key in schema.value) return;
  patch({ [key]: [{ type: "object", properties: {} }] });
}

function updateComposition(key: CompositionKey, index: number, value: OpenApiObject) {
  const branches = schema.value[key];
  if (!Array.isArray(branches)) return;
  patch({ [key]: branches.map((branch, branchIndex) => (branchIndex === index ? value : branch)) });
}

function addCompositionBranch(key: CompositionKey) {
  const branches = schema.value[key];
  if (Array.isArray(branches)) patch({ [key]: [...branches, { type: "object", properties: {} }] });
}

function removeCompositionBranch(key: CompositionKey, index: number) {
  const branches = schema.value[key];
  if (!canEdit.value || !Array.isArray(branches)) return;
  const next = { ...schema.value };
  const remaining = branches.filter((_, branchIndex) => branchIndex !== index);
  if (remaining.length) next[key] = remaining;
  else delete next[key];
  emit("update:modelValue", next);
}

function addProperty() {
  let index = properties.value.length + 1;
  let key = `property${index}`;
  while (Object.hasOwn(schema.value.properties ?? {}, key)) {
    index += 1;
    key = `property${index}`;
  }
  patch({ properties: { ...(isRecord(schema.value.properties) ? schema.value.properties : {}), [key]: { type: "string" } } });
}

function renameProperty(oldName: string, newName: string, input: HTMLInputElement) {
  const normalized = newName.trim();
  if (!normalized || normalized === oldName || !isRecord(schema.value.properties)) {
    input.value = oldName;
    return;
  }
  if (Object.hasOwn(schema.value.properties, normalized)) {
    input.value = oldName;
    return;
  }
  const nextProperties = Object.fromEntries(Object.entries(schema.value.properties).map(([name, value]) => [name === oldName ? normalized : name, value]));
  const nextRequired = required.value.map((name) => (name === oldName ? normalized : name));
  emit("update:modelValue", { ...schema.value, properties: nextProperties, ...(nextRequired.length ? { required: nextRequired } : {}) });
}

function removeProperty(name: string) {
  if (!canEdit.value || !isRecord(schema.value.properties)) return;
  const nextProperties = { ...schema.value.properties };
  delete nextProperties[name];
  const nextRequired = required.value.filter((item) => item !== name);
  const next: OpenApiObject = { ...schema.value, properties: nextProperties };
  if (nextRequired.length) next.required = nextRequired;
  else delete next.required;
  emit("update:modelValue", next);
}

function toggleRequired(name: string, value: boolean) {
  if (!canEdit.value) return;
  const nextRequired = new Set(required.value);
  if (value) nextRequired.add(name);
  else nextRequired.delete(name);
  const next = { ...schema.value };
  if (nextRequired.size) next.required = [...nextRequired];
  else delete next.required;
  emit("update:modelValue", next);
}

function updateChild(name: string, value: OpenApiObject) {
  if (!canEdit.value || !isRecord(schema.value.properties)) return;
  emit("update:modelValue", { ...schema.value, properties: { ...schema.value.properties, [name]: value } });
}

function propertySchema(value: unknown): OpenApiObject {
  if (!isRecord(value)) return {};
  return JSON.parse(JSON.stringify(materializeOpenApiSchema(props.document, value))) as OpenApiObject;
}

function propertyType(value: unknown): string {
  const child = propertySchema(value);
  if (typeof child.type === "string") return child.type;
  if (Array.isArray(child.type)) {
    const nonNullTypes = child.type.filter((item): item is string => typeof item === "string" && item !== "null");
    if (nonNullTypes.length === 1) return nonNullTypes[0]!;
  }
  if (isRecord(child.properties)) return "object";
  if (child.items) return "array";
  if (typeof child.$ref === "string") return child.$ref.split("/").at(-1) ?? "object";
  return "string";
}

function propertyValueText(value: unknown): string {
  if (value === undefined || value === null) return "";
  return typeof value === "string" ? value : JSON.stringify(value);
}

function propertyString(value: unknown, key: string): string {
  const child = propertySchema(value);
  return typeof child[key] === "string" ? (child[key] as string) : "";
}

function toggleProperty(name: string) {
  const next = new Set(expandedProperties.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  expandedProperties.value = next;
}

function updatePropertyField(name: string, key: string, value: unknown) {
  if (!canEdit.value || !isRecord(schema.value.properties) || !isRecord(schema.value.properties[name])) return;
  const child = propertySchema(schema.value.properties[name]);
  if (value === "" || value === undefined) delete child[key];
  else child[key] = value;
  updateChild(name, child);
}

function changePropertyExample(name: string, input: HTMLInputElement) {
  const value = input.value.trim();
  if (!value) {
    updatePropertyField(name, "example", undefined);
    return;
  }
  const child = isRecord(schema.value.properties) ? propertySchema(schema.value.properties[name]) : {};
  const type = propertyType(child);
  let parsed: unknown = value;
  if (type === "integer" || type === "number") {
    parsed = Number(value);
    if (!Number.isFinite(parsed) || (type === "integer" && !Number.isInteger(parsed))) return;
  } else if (type === "boolean") {
    if (!["true", "false"].includes(value.toLowerCase())) return;
    parsed = value.toLowerCase() === "true";
  } else if (type === "object" || type === "array") {
    try {
      parsed = JSON.parse(value);
    } catch {
      return;
    }
  }
  updatePropertyField(name, "example", parsed);
}

function updateItems(value: OpenApiObject) {
  patch({ items: value });
}
</script>

<template>
  <section class="schema-design-node" :class="{ 'schema-design-node-nested': props.depth > 0, 'schema-design-node-compact': props.compact }" :aria-label="props.label">
    <div v-if="!hideRootControls && (sourceReference || schemaReference)" class="schema-design-reference-source">
      <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
        <Layers class="size-3.5 shrink-0" aria-hidden="true" /><code>{{ sourceReference || schemaReference }}</code>
      </div>
      <div v-if="props.editable" class="flex shrink-0 flex-wrap gap-1">
        <Button v-if="sourceReference && !schemaReference" size="xs" variant="ghost" @click="detachModel"><Unlink class="size-3" aria-hidden="true" />{{ t("specification.openapiModels.detach") }}</Button>
      </div>
    </div>
    <p v-if="schemaReference" class="schema-design-reference">{{ t("specification.openapiDesignNestedReferenceReadonly", { ref: schemaReference }) }}</p>
    <p v-else-if="sourceReference" class="schema-design-reference">{{ t("specification.openapiDesignSharedSchemaNotice", { ref: sourceReference }) }}</p>
    <div v-if="!props.hideRootControls" class="schema-design-controls">
      <span v-if="props.compact" class="schema-design-root-label">{{ t("specification.openapiSchemaRoot") }}</span>
      <OpenApiSchemaFieldType :model-value="props.modelValue" :name="`schema-type-${props.depth}-${props.label}`" :field-name="`${props.depth}-${props.label}`" :label="props.label" :document="props.document" :disabled="!props.editable" @update:model-value="emit('update:modelValue', $event)" />
      <label class="schema-design-control schema-design-description">
        <span :class="{ 'sr-only': props.compact }">{{ t("specification.openapiDescription") }}</span>
        <textarea
          :value="typeof schema.description === 'string' ? schema.description : ''"
          class="field-control"
          :placeholder="props.compact ? t('specification.openapiDescription') : undefined"
          :disabled="!canEdit"
          :name="`schema-description-${props.depth}-${props.label}`"
          rows="1"
          @input="patch({ description: ($event.target as HTMLTextAreaElement).value || undefined })"
        />
      </label>
    </div>
    <details v-if="!schemaReference && canEdit && props.depth < 8" class="schema-design-structure" :open="!props.compact">
      <summary>{{ t("specification.openapiSchemaStructureSettings") }}</summary>
      <div class="schema-design-structure-settings">
        <section v-if="valueType === 'object' && props.depth < 8" class="schema-design-additional">
          <label class="schema-design-control">
            <span>{{ t("specification.openapiSchemaAdditionalProperties") }}</span>
            <AppSelect
              :value="isRecord(schema.additionalProperties) ? 'schema' : schema.additionalProperties === false ? 'forbidden' : 'allowed'"
              :name="`schema-additionalProperties-${props.depth}-${props.label}`"
              size="compact"
              class="field-control"
              :disabled="!canEdit"
              @change="setAdditionalProperties(($event.target as HTMLSelectElement).value)"
            >
              <option value="allowed">{{ t("specification.openapiSchemaAdditionalAllowed") }}</option>
              <option value="forbidden">{{ t("specification.openapiSchemaAdditionalForbidden") }}</option>
              <option value="schema">{{ t("specification.openapiSchemaAdditionalTyped") }}</option>
            </AppSelect>
          </label>
          <OpenApiSchemaDesignEditor
            v-if="isRecord(schema.additionalProperties)"
            :model-value="schema.additionalProperties"
            :label="t('specification.openapiAdditionalProperties')"
            :editable="canEdit"
            :depth="props.depth + 1"
            :document="props.document"
            :ancestor-references="childReferences"
            @update:model-value="patch({ additionalProperties: $event })"
          />
        </section>
        <label v-if="compositions.length < compositionKeys.length && props.depth < 8" class="schema-design-control">
          <span>{{ t("specification.openapiSchemaAddComposition") }}</span>
          <AppSelect
            :value="''"
            :name="`schema-composition-${props.depth}-${props.label}`"
            size="compact"
            class="field-control"
            :disabled="!canEdit"
            @change="
              addComposition(($event.target as HTMLSelectElement).value);
              ($event.target as HTMLSelectElement).value = '';
            "
          >
            <option value="">{{ t("specification.openapiSchemaSelectComposition") }}</option>
            <option v-for="key in compositionKeys.filter((key) => !(key in schema))" :key="key" :value="key">{{ key }}</option>
          </AppSelect>
        </label>
      </div>
    </details>
    <section v-if="!schemaReference && valueType === 'object' && props.depth < 8" class="schema-design-properties">
      <header class="schema-design-section-heading">
        <div>
          <strong>{{ t("specification.openapiSchemaProperties") }}</strong
          ><span>{{ properties.length }}</span>
        </div>
        <button type="button" class="debug-add-button" :disabled="!canEdit" @click="addProperty"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiSchemaAddProperty") }}</button>
      </header>
      <div v-if="properties.length" class="schema-design-property-list">
        <div class="schema-design-property-scroll" role="region" tabindex="0" :aria-label="t('specification.openapiSchemaProperties')">
          <table class="schema-design-property-table">
            <caption class="sr-only">
              {{
                t("specification.openapiSchemaProperties")
              }}
            </caption>
            <thead>
              <tr>
                <th scope="col">{{ t("specification.openapiSchemaPropertyName") }}</th>
                <th scope="col">{{ t("specification.openapiType") }}</th>
                <th scope="col">{{ t("specification.openapiSchemaExample") }}</th>
                <th scope="col">{{ t("specification.openapiSchemaTitle") }}</th>
                <th scope="col">{{ t("specification.openapiDescription") }}</th>
                <th scope="col">
                  <span class="sr-only">{{ t("specification.openapiSchemaPropertyActions") }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <template v-for="[name, child] in properties" :key="name">
                <tr class="schema-design-property">
                  <td>
                    <input
                      :value="name"
                      class="field-control schema-design-property-name"
                      :aria-label="t('specification.openapiSchemaPropertyName')"
                      :disabled="!canEdit"
                      autocomplete="off"
                      spellcheck="false"
                      @change="renameProperty(name, ($event.target as HTMLInputElement).value, $event.target as HTMLInputElement)"
                    />
                  </td>
                  <td>
                    <OpenApiSchemaFieldType
                      v-if="isRecord(child)"
                      :model-value="child"
                      :name="`schema-property-type-${props.depth}-${name}`"
                      :field-name="`${props.depth + 1}-${name}`"
                      :label="name"
                      :document="props.document"
                      :disabled="!canEdit"
                      :required="required.includes(name)"
                      @update:required="toggleRequired(name, $event ?? false)"
                      @update:model-value="updateChild(name, $event)"
                    />
                    <code v-else>{{ propertyType(child) }}</code>
                    <span v-if="isRecord(child) && childReferences.includes(schemaModelReference(child))" class="schema-design-property-reference">{{ t("specification.openapiDesignNestedReferenceReadonly", { ref: schemaModelReference(child) }) }}</span>
                  </td>
                  <td>
                    <input
                      v-if="isRecord(child)"
                      :value="propertyValueText(propertySchema(child).example)"
                      class="field-control schema-design-property-value"
                      :aria-label="`${t('specification.openapiSchemaExample')} ${name}`"
                      :disabled="!canEdit || Boolean(schemaModelReference(child))"
                      autocomplete="off"
                      @change="changePropertyExample(name, $event.target as HTMLInputElement)"
                    />
                  </td>
                  <td>
                    <input
                      v-if="isRecord(child)"
                      :value="propertyString(child, 'title')"
                      class="field-control schema-design-property-value"
                      :aria-label="`${t('specification.openapiSchemaTitle')} ${name}`"
                      :disabled="!canEdit || Boolean(schemaModelReference(child))"
                      autocomplete="off"
                      @input="updatePropertyField(name, 'title', ($event.target as HTMLInputElement).value)"
                    />
                  </td>
                  <td>
                    <textarea
                      v-if="isRecord(child)"
                      :value="propertyString(child, 'description')"
                      :name="`schema-description-${props.depth + 1}-${name}`"
                      class="field-control schema-design-property-description"
                      :aria-label="`${t('specification.openapiDescription')} ${name}`"
                      :disabled="!canEdit || Boolean(schemaModelReference(child))"
                      rows="1"
                      @input="updatePropertyField(name, 'description', ($event.target as HTMLTextAreaElement).value)"
                    />
                  </td>
                  <td class="schema-design-property-actions">
                    <button
                      v-if="isRecord(child)"
                      type="button"
                      class="icon-button"
                      :aria-label="expandedProperties.has(name) ? t('specification.openapiCollapseFields') : t('specification.openapiExpandFields')"
                      :aria-expanded="expandedProperties.has(name)"
                      :disabled="!canEdit"
                      @click="toggleProperty(name)"
                    >
                      <span aria-hidden="true">{{ expandedProperties.has(name) ? "−" : "+" }}</span>
                    </button>
                    <button type="button" class="icon-button" :aria-label="t('specification.openapiSchemaRemoveProperty', { name })" :disabled="!canEdit" @click="removeProperty(name)"><Trash2 class="h-3.5 w-3.5" /></button>
                  </td>
                </tr>
                <tr v-if="expandedProperties.has(name) && isRecord(child)" class="schema-design-property-details">
                  <td colspan="6">
                    <OpenApiSchemaDesignEditor :model-value="child" :label="name" :editable="canEdit" :depth="props.depth + 1" :document="props.document" :ancestor-references="childReferences" @update:model-value="updateChild(name, $event)" />
                  </td>
                </tr>
              </template>
            </tbody>
          </table>
        </div>
      </div>
      <p v-else class="schema-design-empty">{{ t("specification.openapiSchemaNoProperties") }}</p>
    </section>
    <section v-if="!schemaReference && valueType === 'array' && props.depth < 8" class="schema-design-items">
      <strong>{{ t("specification.openapiItems") }}</strong>
      <OpenApiSchemaDesignEditor v-if="isRecord(schema.items)" :model-value="schema.items" :label="t('specification.openapiSchemaItem')" :editable="canEdit" :depth="props.depth + 1" :document="props.document" :ancestor-references="childReferences" @update:model-value="updateItems" />
      <button v-else type="button" class="debug-add-button" :disabled="!canEdit" @click="updateItems({ type: 'string' })"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiSchemaDefineItems") }}</button>
    </section>
    <template v-if="!schemaReference && props.depth < 8">
      <section v-for="composition in compositions" :key="composition.key" class="schema-design-composition">
        <header class="schema-design-section-heading">
          <strong>{{ composition.key }} · {{ t(`specification.openapiSchemaComposition${composition.key === "allOf" ? "All" : composition.key === "anyOf" ? "Any" : "One"}`) }}</strong>
          <button type="button" class="debug-add-button" :disabled="!canEdit" @click="addCompositionBranch(composition.key)"><Plus class="h-3.5 w-3.5" />{{ t("specification.openapiSchemaAddBranch") }}</button>
        </header>
        <article v-for="(branch, index) in composition.branches" :key="index" class="schema-design-branch">
          <header class="schema-design-section-heading">
            <strong>{{ t("specification.openapiSchemaVariantItem", { index: index + 1 }) }}</strong>
            <button type="button" class="icon-button" :aria-label="t('specification.openapiSchemaRemoveBranch', { index: index + 1 })" :disabled="!canEdit" @click="removeCompositionBranch(composition.key, index)"><Trash2 class="h-3.5 w-3.5" /></button>
          </header>
          <OpenApiSchemaDesignEditor
            v-if="isRecord(branch)"
            :model-value="branch"
            :label="`${composition.key}-${index + 1}`"
            :editable="canEdit"
            :depth="props.depth + 1"
            :document="props.document"
            :ancestor-references="childReferences"
            @update:model-value="updateComposition(composition.key, index, $event)"
          />
          <code v-else>{{ JSON.stringify(branch) }}</code>
        </article>
      </section>
    </template>
    <p v-if="hasUnsupportedComposition" class="schema-design-reference">{{ t("specification.openapiSchemaUnsupportedTypeReadonly") }}</p>
  </section>
</template>
