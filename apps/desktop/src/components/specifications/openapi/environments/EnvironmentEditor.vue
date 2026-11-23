<script setup lang="ts">
import { nextTick, ref } from "vue";
import { Button } from "@/components/ui/button";
import { translate as t } from "@/i18n";
import type { EnvironmentDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";
import OpenApiEnvironmentServers from "./OpenApiEnvironmentServers.vue";
import EnvironmentVariablesTable from "./EnvironmentVariablesTable.vue";

export type { EnvironmentDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";
defineProps<{ busy: boolean; error?: string; saved?: boolean }>();
const draft = defineModel<EnvironmentDraft>({ required: true });
const emit = defineEmits<{ save: [close: boolean]; dirty: [] }>();
const form = ref<HTMLFormElement>();
const invalid = ref("");
const invalidField = ref("");

function clearError() {
  invalid.value = "";
  invalidField.value = "";
  emit("dirty");
}

function validUrl(value: string) {
  const candidate = value.trim().replace(/\{\{[^}]+\}\}|\{[^}]+\}/g, "example");
  try {
    return /^https?:\/\//i.test(candidate) && !!new URL(candidate).hostname;
  } catch {
    return false;
  }
}

async function submit(close: boolean) {
  invalid.value = "";
  invalidField.value = "";
  const fail = (message: string, field: string) => {
    invalid.value = message;
    invalidField.value = field;
  };
  if (!draft.value.name.trim()) fail(t("environmentContext.nameRequired"), "environment-name");
  else if (draft.value.baseUrl.trim() && !validUrl(draft.value.baseUrl) && !/^\/(?!\/)/.test(draft.value.baseUrl.trim())) fail(t("environmentContext.urlInvalid"), "environment-base-url");
  const serverNames = new Set<string>();
  for (const server of draft.value.servers ?? []) {
    if (invalid.value) break;
    const name = server.name.trim();
    if (!name || serverNames.has(name)) fail(t("environmentContext.serverInvalid"), `environment-server-name-${server.id}`);
    else if (!validUrl(server.baseUrl)) fail(t("environmentContext.urlInvalid"), `environment-server-url-${server.id}`);
    serverNames.add(name);
  }
  const variableNames = new Set<string>();
  for (const [index, row] of draft.value.variables.entries()) {
    if (invalid.value) break;
    const key = row.key.trim();
    if (!key || variableNames.has(key)) fail(t("environmentContext.variableInvalid"), `environment-key-${index}`);
    variableNames.add(key);
  }
  if (invalid.value) {
    await nextTick();
    const field = form.value?.elements.namedItem(invalidField.value);
    if (field instanceof HTMLElement) field.focus();
    return;
  }
  emit("save", close);
}
</script>

<template>
  <form ref="form" class="environment-manager-editor" novalidate @submit.prevent="submit(false)" @input="clearError">
    <div class="environment-editor-content">
      <fieldset :disabled="busy">
        <div class="environment-editor-title">
          <span class="environment-avatar" aria-hidden="true">{{ draft.name.trim().slice(0, 1).toUpperCase() }}</span>
          <input v-model="draft.name" name="environment-name" :placeholder="t('environmentContext.name')" :aria-label="t('environmentContext.name')" :aria-invalid="invalidField === 'environment-name'" maxlength="80" autocomplete="off" />
        </div>
        <OpenApiEnvironmentServers v-model="draft" :invalid-field="invalidField" @dirty="clearError" />
        <EnvironmentVariablesTable v-model="draft.variables" :invalid-field="invalidField" @dirty="clearError" />
      </fieldset>
    </div>
    <div class="environment-editor-footer">
      <p v-if="invalid || error" role="alert" class="text-destructive">{{ invalid || error }}</p>
      <p v-else-if="saved" role="status">{{ t("environmentContext.saved") }}</p>
      <div class="environment-editor-buttons">
        <Button type="button" size="sm" variant="secondary" :disabled="busy" @click="submit(true)">{{ t("requestAgent.saveClose") }}</Button>
        <Button type="submit" size="sm" :disabled="busy">{{ t(busy ? "common.saving" : "common.save") }}</Button>
      </div>
    </div>
  </form>
</template>

<style scoped>
.environment-manager-editor {
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  min-height: 0;
  min-width: 0;
}
.environment-editor-content {
  min-width: 0;
  overflow: auto;
  overscroll-behavior: contain;
  padding: 20px;
}
.environment-editor-content fieldset {
  min-inline-size: 0;
  display: grid;
  gap: 22px;
}
.environment-editor-title {
  display: flex;
  align-items: center;
  gap: 8px;
}
.environment-editor-title input {
  width: 100%;
  min-width: 0;
  padding: 5px 7px;
  border: 1px solid transparent;
  border-radius: 5px;
  font-size: 16px;
  font-weight: 500;
}
.environment-editor-title input:hover,
.environment-editor-title input:focus-visible {
  border-color: var(--border);
}
.environment-editor-footer {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 10px 20px 16px;
}
.environment-editor-footer p {
  flex: 1 1 180px;
  font-size: 12px;
}
.environment-editor-buttons {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
  margin-inline-start: auto;
}
@media (max-width: 640px) {
  .environment-editor-content,
  .environment-editor-footer {
    padding: 10px;
  }
}
</style>
