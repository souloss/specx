<script setup lang="ts">
import { ref, useId } from "vue";
import { Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate as t } from "@/i18n";
import { copyPublicEnvironment } from "@/lib/specifications/openapi/environments/openapiPublicEnvironments";
import type { OpenApiEnvironment, PublicOpenApiEnvironment } from "@/types/api";

const props = withDefaults(defineProps<{ sources?: OpenApiEnvironment[]; disabled?: boolean; valid: boolean }>(), { sources: () => [] });
const environments = defineModel<PublicOpenApiEnvironment[]>({ required: true });
const sourceId = ref("");
const id = useId();
function add() {
  const source = props.sources.find((item) => item.id === sourceId.value);
  environments.value = [...environments.value, source ? copyPublicEnvironment(source) : { name: "", baseUrl: "", variables: [] }];
  sourceId.value = "";
}
</script>

<template>
  <fieldset class="public-environment-editor space-y-3 rounded-lg border border-border p-3 sm:p-4" :disabled="disabled">
    <legend class="px-1 text-sm font-semibold">{{ t("openapiPublication.environments") }}</legend>
    <p class="text-xs leading-5 text-muted-foreground">{{ t("openapiPublication.environmentsHelp") }}</p>
    <div class="flex flex-wrap items-center gap-2">
      <AppSelect v-model="sourceId" size="form" class="field-control min-w-0 flex-1" :aria-label="t('openapiPublication.environmentSource')" name="public-environment-source" :disabled="disabled || environments.length >= 20">
        <option value="">{{ t("openapiPublication.customEnvironment") }}</option>
        <option v-for="source in sources" :key="source.id" :value="source.id">{{ source.name }}</option>
      </AppSelect>
      <Button size="sm" variant="outline" :disabled="disabled || environments.length >= 20" @click="add"><Plus class="size-3.5" aria-hidden="true" />{{ t("openapiPublication.addEnvironment") }}</Button>
    </div>
    <p v-if="!environments.length" class="text-xs text-muted-foreground">{{ t("openapiPublication.environmentsEmpty") }}</p>
    <div v-for="(environment, index) in environments" :key="index" class="space-y-3 rounded-md border border-border bg-muted/20 p-3">
      <div class="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-2">
        <div class="grid min-w-0 gap-3 sm:grid-cols-[minmax(8rem,1fr)_minmax(0,2fr)]">
          <label class="grid min-w-0 gap-1.5 text-xs">{{ t("openapiPublication.environmentName") }}<input v-model="environment.name" :name="`${id}-environment-${index}-name`" class="field-control w-full" maxlength="80" autocomplete="off" /></label>
          <label class="grid min-w-0 gap-1.5 text-xs"
            >{{ t("openapiPublication.publicBaseUrl") }}<input v-model="environment.baseUrl" :name="`${id}-environment-${index}-url`" class="field-control w-full font-mono" maxlength="2000" autocomplete="off" spellcheck="false" placeholder="https://api.example.com…"
          /></label>
        </div>
        <Button size="icon" variant="ghost" :aria-label="t('openapiPublication.removeEnvironment', { name: environment.name || String(index + 1) })" @click="environments.splice(index, 1)"><Trash2 class="size-4" aria-hidden="true" /></Button>
      </div>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <span class="text-xs font-medium">{{ t("openapiPublication.publicVariables") }}</span>
        <Button size="xs" variant="ghost" :disabled="disabled || environment.variables.length >= 100" @click="environment.variables.push({ key: '', value: '' })"><Plus class="size-3" aria-hidden="true" />{{ t("openapi.addVariable") }}</Button>
      </div>
      <div
        v-for="(variable, variableIndex) in environment.variables"
        :key="variableIndex"
        class="public-environment-variable-row grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-md border border-border p-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] sm:rounded-none sm:border-0 sm:p-0"
      >
        <input v-model="variable.key" :name="`${id}-environment-${index}-key-${variableIndex}`" class="field-control w-full font-mono" :aria-label="t('openapi.variableName')" maxlength="120" autocomplete="off" spellcheck="false" :placeholder="t('openapi.variableName')" />
        <input
          v-model="variable.value"
          :name="`${id}-environment-${index}-value-${variableIndex}`"
          class="field-control col-start-1 row-start-2 w-full font-mono sm:col-start-auto sm:row-start-auto"
          :aria-label="t('openapi.variableValue')"
          autocomplete="off"
          spellcheck="false"
          :placeholder="t('openapi.variableValue')"
        />
        <Button size="icon" variant="ghost" class="col-start-2 row-span-2 row-start-1 sm:col-start-auto sm:row-span-1 sm:row-start-auto" :aria-label="t('openapiPublication.removeVariable', { name: variable.key || String(variableIndex + 1) })" @click="environment.variables.splice(variableIndex, 1)"
          ><Trash2 class="size-3.5" aria-hidden="true"
        /></Button>
      </div>
    </div>
    <p v-if="!valid" role="alert" class="text-xs leading-5 text-destructive">{{ t("openapiPublication.environmentInvalid") }}</p>
  </fieldset>
</template>
