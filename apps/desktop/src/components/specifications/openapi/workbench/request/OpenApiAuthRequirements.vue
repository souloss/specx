<script setup lang="ts">
import AppSelect from "@/components/ui/AppSelect.vue";
const authTemplateExample = "{{TOKEN}}";
import { ref } from "vue";
import { Eye, EyeOff } from "@lucide/vue";
import { translate as t } from "@/i18n";
import { authSchemeSupported, authVariableNames, emptyAuthInput, type OpenApiAuthInput, type OpenApiAuthScheme } from "@/lib/specifications/openapi/workbench/request/openapiAuth";
import type { OpenApiEnvironmentVariable } from "@/types/api";
const props = defineProps<{
  requirements: Record<string, unknown>[];
  schemes: OpenApiAuthScheme[];
  alternative: number;
  inputs: Record<string, OpenApiAuthInput>;
  variables: OpenApiEnvironmentVariable[];
  tokenSchemes: string[];
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:alternative": [value: number]; "update:inputs": [value: Record<string, OpenApiAuthInput>]; authorize: [name: string] }>();
const visible = ref<Record<string, boolean>>({});
function fields(scheme: Record<string, unknown>): Array<keyof OpenApiAuthInput> {
  return scheme.type === "http" && String(scheme.scheme).toLowerCase() === "basic" ? ["username", "password"] : ["value"];
}
function input(name: string): OpenApiAuthInput {
  return Object.hasOwn(props.inputs, name) ? props.inputs[name]! : emptyAuthInput();
}
function setInput(name: string, field: keyof OpenApiAuthInput, event: Event) {
  emit("update:inputs", { ...props.inputs, [name]: { ...input(name), [field]: (event.target as HTMLInputElement).value } });
}
function availableVariables(name: string, field: keyof OpenApiAuthInput, scheme: Record<string, unknown>): string {
  const aliases = authVariableNames(name, field, scheme).map((value) => value.toLowerCase());
  return props.variables
    .filter((variable) => variable.enabled && variable.hasValue && aliases.includes(variable.key.toLowerCase()))
    .map((variable) => variable.key)
    .join(", ");
}
function label(field: keyof OpenApiAuthInput): string {
  return t(field === "username" ? "specification.openapiAuth.username" : field === "password" ? "specification.openapiAuth.password" : "specification.openapiAuth.value");
}
</script>

<template>
  <div class="openapi-auth-requirements">
    <label v-if="requirements.length > 1" class="openapi-auth-alternative">
      <span>{{ t("specification.openapiAuth.alternative") }}</span>
      <AppSelect size="form" class="field-control" name="openapi-auth-alternative" :value="alternative" :disabled="disabled" @change="emit('update:alternative', Number(($event.target as HTMLSelectElement).value))">
        <option v-for="(requirement, index) in requirements" :key="index" :value="index">{{ Object.keys(requirement).join(" + ") || t("specification.openapiNoAuth") }}</option>
      </AppSelect>
    </label>
    <p class="openapi-auth-help">{{ Object.keys(requirements[alternative] ?? {}).length ? t("specification.openapiAuth.combination") : t("specification.openapiAuth.anonymous") }}</p>
    <section v-for="name in Object.keys(requirements[alternative] ?? {})" :key="name" class="openapi-auth-card" :aria-label="name">
      <template v-for="scheme in [schemes.find((item) => item.name === name)?.scheme]" :key="name">
        <div class="openapi-auth-card-heading">
          <strong>{{ name }}</strong
          ><code v-if="scheme" translate="no"
            >{{ scheme.type }}<template v-if="scheme.in"> · {{ scheme.in }} · {{ scheme.name }}</template
            ><template v-else-if="scheme.scheme"> · {{ scheme.scheme }}</template></code
          >
        </div>
        <p v-if="!scheme || !authSchemeSupported(scheme)" class="text-xs text-destructive" role="alert">{{ t("specification.openapiAuth.unsupported", { scheme: name }) }}</p>
        <template v-else>
          <p v-if="scheme.description" class="openapi-auth-help">{{ scheme.description }}</p>
          <div v-for="field in fields(scheme)" :key="field" class="openapi-auth-input">
            <label :for="`openapi-auth-${encodeURIComponent(name)}-${field}`">{{ label(field) }}</label>
            <div class="openapi-auth-secret">
              <input
                :id="`openapi-auth-${encodeURIComponent(name)}-${field}`"
                :name="`openapi-auth-${encodeURIComponent(name)}-${field}`"
                class="field-control"
                :type="field === 'username' || visible[`${name}:${field}`] ? 'text' : 'password'"
                :value="input(name)[field]"
                :disabled="disabled"
                autocomplete="off"
                spellcheck="false"
                :placeholder="t('specification.openapiAuth.inputPlaceholder', { example: authTemplateExample })"
                @input="setInput(name, field, $event)"
              />
              <button
                v-if="field !== 'username'"
                type="button"
                class="icon-button"
                :aria-label="t(visible[`${name}:${field}`] ? 'specification.openapiAuth.hide' : 'specification.openapiAuth.show', { name, field: label(field) })"
                :aria-pressed="Boolean(visible[`${name}:${field}`])"
                :disabled="disabled"
                @click="visible[`${name}:${field}`] = !visible[`${name}:${field}`]"
              >
                <EyeOff v-if="visible[`${name}:${field}`]" class="h-3.5 w-3.5" aria-hidden="true" /><Eye v-else class="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </div>
            <small v-if="availableVariables(name, field, scheme)">{{ t("specification.openapiAuth.environmentSource", { names: availableVariables(name, field, scheme) }) }}</small>
            <small v-else>{{ t("specification.openapiAuth.environmentHint", { names: authVariableNames(name, field, scheme).slice(0, 2).join(" / ") }) }}</small>
          </div>
          <div v-if="scheme.type === 'oauth2' || scheme.type === 'openIdConnect'" class="openapi-auth-oauth-actions">
            <span v-if="tokenSchemes.includes(name)" class="text-xs text-emerald-600">{{ t("specification.openapiOAuthTokenReady") }}</span>
            <button type="button" class="debug-add-button" :disabled="disabled" @click="emit('authorize', name)">{{ t("specification.openapiAuth.openOAuth") }}</button>
          </div>
          <p v-if="scheme.in === 'cookie'" class="openapi-auth-help">{{ t("specification.openapiAuth.cookieLimit") }}</p>
        </template>
      </template>
    </section>
    <p v-if="Object.keys(requirements[alternative] ?? {}).length" class="openapi-auth-help">{{ t("specification.openapiAuth.persistence", { example: authTemplateExample }) }}</p>
  </div>
</template>
