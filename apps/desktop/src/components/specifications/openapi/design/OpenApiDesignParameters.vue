<script setup lang="ts">
import { Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppSelect from "@/components/ui/AppSelect.vue";
import OpenApiSchemaFieldType from "./OpenApiSchemaFieldType.vue";
import { translate as t } from "@/i18n";
import { type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import type { ParameterDraft } from "@/lib/specifications/openapi/design/openapiDesignDraft";
import OpenApiSchemaDesignEditor from "./OpenApiSchemaDesignEditor.vue";

const props = defineProps<{
  editable: boolean;
  rows: ParameterDraft[];
  location: string;
  heading: string;
  parsedDocument: OpenApiObject | null;
  pathName: (row: ParameterDraft) => string;
  parameterSchema: (row: ParameterDraft) => OpenApiObject;
  parameterStyles: (location: string) => string[];
  invalidParameterKey?: string | null;
}>();
const emit = defineEmits<{ add: [name?: string]; remove: [row: ParameterDraft]; location: [row: ParameterDraft, location: string]; schema: [row: ParameterDraft, schema: OpenApiObject] }>();
</script>

<template>
  <section class="openapi-design-parameter-group" :data-parameter-location="location">
    <header class="openapi-design-subsection-heading">
      <h5>{{ heading }}</h5>
      <Button v-if="location !== 'path'" type="button" size="xs" variant="outline" :disabled="!props.editable" @click="emit('add')">
        <Plus class="size-3.5" aria-hidden="true" />
        {{ t("specification.openapiDesignAddParameterFor", { location: heading }) }}
      </Button>
    </header>
    <div v-if="rows.length" class="openapi-design-table-wrap" role="region" tabindex="0" :aria-label="heading">
      <table class="openapi-design-table">
        <caption class="sr-only">
          {{
            heading
          }}
        </caption>
        <thead>
          <tr role="row">
            <th scope="col">{{ t("specification.openapiName") }}</th>
            <th scope="col">{{ t("specification.openapiType") }}</th>
            <th scope="col">{{ t("specification.openapiExample") }}</th>
            <th scope="col">{{ t("specification.openapiDescription") }}</th>
            <th scope="col">
              <span class="sr-only">{{ t("specification.openapiDesignParameterActions") }}</span>
            </th>
          </tr>
        </thead>
        <tbody role="rowgroup">
          <template v-for="parameter in rows" :key="parameter.key"
            ><tr role="row">
              <td role="cell" :data-label="t('specification.openapiName')">
                <div class="openapi-design-name-cell">
                  <input
                    v-if="parameter.editable && parameter.location !== 'path'"
                    :id="`design-parameter-${parameter.key}`"
                    v-model="parameter.name"
                    class="field-control"
                    :name="`parameter-name-${parameter.key}`"
                    :aria-label="t('specification.openapiName')"
                    :aria-invalid="props.invalidParameterKey === parameter.key"
                    :aria-describedby="props.invalidParameterKey === parameter.key ? 'openapi-design-parameter-error' : undefined"
                    :disabled="!props.editable"
                    autocomplete="off"
                    spellcheck="false"
                  />
                  <code v-else class="openapi-design-parameter-control">{{ parameter.location === "path" ? pathName(parameter) : parameter.name }}</code>
                  <AppSelect
                    v-if="parameter.editable && parameter.location !== 'path'"
                    :value="parameter.location"
                    size="dense"
                    variant="quiet"
                    class="openapi-design-parameter-location"
                    :aria-label="t('specification.openapiLocation')"
                    :disabled="!props.editable"
                    @change="emit('location', parameter, ($event.target as HTMLSelectElement).value)"
                  >
                    <option value="query">query</option>
                    <option value="header">header</option>
                    <option value="cookie">cookie</option>
                  </AppSelect>
                  <span v-else class="openapi-design-location-badge">{{ parameter.location }}</span>
                </div>
              </td>
              <td role="cell" :data-label="t('specification.openapiType')">
                <OpenApiSchemaFieldType
                  v-if="parameter.editable"
                  :model-value="parameterSchema(parameter)"
                  v-model:required="parameter.required"
                  v-model:deprecated="parameter.deprecated"
                  :name="`parameter-type-${parameter.key}`"
                  :field-name="`parameter-${parameter.key}`"
                  :label="parameter.name"
                  :document="parsedDocument"
                  :primitive-only="parameter.location === 'path'"
                  :required-locked="parameter.location === 'path'"
                  :disabled="!props.editable"
                  :serialization="parameter"
                  @update:model-value="emit('schema', parameter, $event)"
                  @serialization="Object.assign(parameter, $event)"
                >
                  <template #structure="{ schema, update }">
                    <OpenApiSchemaDesignEditor
                      v-if="!parameter.type.startsWith('#/') && (schema.type === 'object' || schema.type === 'array' || (Array.isArray(schema.type) && (schema.type.includes('object') || schema.type.includes('array'))))"
                      :model-value="schema"
                      :label="parameter.name"
                      :document="parsedDocument"
                      hide-root-controls
                      @update:model-value="update"
                    />
                  </template>
                  <template #serialization="{ value }">
                    <div class="schema-constraint-grid border-t border-border pt-3">
                      <label class="schema-design-control">
                        <span>style</span>
                        <AppSelect v-model="value.style" size="compact" :name="`parameter-style-${parameter.key}`" aria-label="style">
                          <option value="">{{ t("specification.openapiDesignSerializationDefault") }}</option>
                          <option v-for="style in parameterStyles(parameter.location)" :key="style" :value="style">{{ style }}</option>
                        </AppSelect>
                      </label>
                      <label class="schema-design-control">
                        <span>explode</span>
                        <AppSelect v-model="value.explode" size="compact" :name="`parameter-explode-${parameter.key}`" aria-label="explode">
                          <option value="">{{ t("specification.openapiDesignSerializationDefault") }}</option>
                          <option value="true">true</option>
                          <option value="false">false</option>
                        </AppSelect>
                      </label>
                      <label v-if="parameter.location === 'query'" class="schema-constraint-toggle col-span-full"> <input v-model="value.allowReserved" type="checkbox" :name="`parameter-allow-reserved-${parameter.key}`" />allowReserved </label>
                    </div>
                  </template>
                </OpenApiSchemaFieldType>
                <code v-else class="openapi-design-parameter-control">{{ parameter.type }}<span v-if="parameter.required" class="schema-field-required text-destructive" :aria-label="t('specification.openapiRequired')"> *</span></code>
              </td>
              <td role="cell" :data-label="t('specification.openapiExample')">
                <input v-model="parameter.example" class="field-control" :name="`parameter-example-${parameter.key}`" :aria-label="t('specification.openapiExample')" :disabled="!props.editable || !parameter.editable" autocomplete="off" spellcheck="false" />
              </td>
              <td role="cell" :data-label="t('specification.openapiDescription')">
                <input v-model="parameter.description" class="field-control" :name="`parameter-description-${parameter.key}`" :aria-label="t('specification.openapiDescription')" :disabled="!props.editable || !parameter.editable" autocomplete="off" />
                <small v-if="parameter.source === 'path'" class="openapi-design-inherited">{{ t("specification.openapiDesignInheritedReadonly") }}</small>
              </td>
              <td role="cell">
                <div v-if="parameter.source === 'operation' && parameter.location !== 'path'" class="openapi-design-parameter-control">
                  <button type="button" class="icon-button" :aria-label="t('specification.openapiDesignRemoveParameter', { name: parameter.name })" :disabled="!props.editable" @click="emit('remove', parameter)">
                    <Trash2 class="h-3.5 w-3.5" aria-hidden="true" />
                  </button>
                </div>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
    <p v-else class="openapi-design-empty">{{ t("specification.openapiDesignNoParametersFor", { location: heading }) }}</p>
  </section>
</template>
