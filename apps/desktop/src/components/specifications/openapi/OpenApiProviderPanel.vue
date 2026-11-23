<script setup lang="ts">
import EnvironmentEditor from "./environments/EnvironmentEditor.vue";
import { environmentDraft as toDraft, environmentExport, environmentInput, type EnvironmentDraft } from "@/lib/specifications/openapi/environments/openapiEnvironments";
import { useEnvironmentLocalValues } from "@/stores/environmentLocalValues";
import AppSelect from "@/components/ui/AppSelect.vue";
import { computed, ref } from "vue";
import { Braces, Download, KeyRound, Plus, Save, Settings2, Trash2, Upload, X } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { createId } from "@/lib/core";
import { translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";
import type { OpenApiEnvironment, OpenApiEnvironmentServer, OpenApiProviderResponse } from "@/types/api";

const props = defineProps<{ serviceId: string; provider: OpenApiProviderResponse }>();
const workspace = useWorkspaceStore();
const t = translate;
const saving = ref(false);
const error = ref("");
const settingsMessage = ref("");
const environmentTransferMessage = ref("");
const environmentImportInput = ref<HTMLInputElement>();
const environmentToDelete = ref<OpenApiEnvironment | null>(null);
const activeEnvironmentId = ref<string | null>(props.provider.environments[0]?.id ?? null);
const settings = ref({
  defaultEnvironmentId: props.provider.settings.defaultEnvironmentId,
  defaultServerUrl: props.provider.settings.defaultServerUrl,
  showOperationId: props.provider.settings.showOperationId,
  schemaDisplay: props.provider.settings.schemaDisplay,
});
const localValues = useEnvironmentLocalValues();
const localScope = (id: string) => `${workspace.catalog?.workspace.id}:${props.serviceId}:${id}`;
const environmentDraft = ref<EnvironmentDraft | null>(null);
const activeEnvironment = computed(() => props.provider.environments.find((environment) => environment.id === activeEnvironmentId.value) ?? null);

function exportEnvironments() {
  const payload = {
    format: "specx-openapi-environments",
    version: 1,
    environments: props.provider.environments.map((environment) => environmentExport(toDraft(environment, localValues.values(localScope(environment.id))))),
  };
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" }));
  link.download = `${props.serviceId}-openapi-environments.json`;
  link.click();
  URL.revokeObjectURL(link.href);
  environmentTransferMessage.value = t("openapi.environmentsExported");
}

function openEnvironmentImport() {
  environmentImportInput.value?.click();
}

async function importEnvironments(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  saving.value = true;
  error.value = "";
  environmentTransferMessage.value = "";
  try {
    const parsed: unknown = JSON.parse(await file.text());
    const source = Array.isArray(parsed) ? parsed : typeof parsed === "object" && parsed !== null && Array.isArray((parsed as { environments?: unknown }).environments) ? (parsed as { environments: unknown[] }).environments : null;
    if (!source) throw new Error(t("openapi.environmentsImportInvalid"));
    let imported = 0;
    for (const value of source) {
      if (!value || typeof value !== "object" || Array.isArray(value)) continue;
      const environment = value as Record<string, unknown>;
      if (typeof environment.name !== "string" || typeof environment.baseUrl !== "string") continue;
      const variables = Array.isArray(environment.variables)
        ? environment.variables.flatMap((item) => {
            if (!item || typeof item !== "object" || Array.isArray(item)) return [];
            const variable = item as Record<string, unknown>;
            if (typeof variable.key !== "string" || !variable.key.trim()) return [];
            return [{ key: variable.key.trim(), localValue: typeof variable.value === "string" && variable.sensitive !== true ? variable.value : undefined, sensitive: variable.sensitive === true, enabled: variable.enabled !== false }];
          })
        : [];
      const servers = Array.isArray(environment.servers)
        ? environment.servers.flatMap((item): OpenApiEnvironmentServer[] => {
            if (!item || typeof item !== "object" || Array.isArray(item)) return [];
            const server = item as Record<string, unknown>;
            if (typeof server.name !== "string" || typeof server.baseUrl !== "string") return [];
            return [{ id: createId("server"), name: server.name, baseUrl: server.baseUrl, requestMode: server.requestMode === "browser" || server.requestMode === "agent" ? server.requestMode : "inherit" }];
          })
        : [];
      const id = createId("openapi-env");
      await workspace.createOpenApiEnvironment(props.serviceId, {
        id,
        name: environment.name,
        slug: typeof environment.slug === "string" ? environment.slug : undefined,
        baseUrl: environment.baseUrl,
        requestMode: environment.requestMode === "browser" || environment.requestMode === "agent" ? environment.requestMode : "inherit",
        servers,
        variables: variables.map(({ key, enabled, sensitive }) => ({ key, enabled, sensitive })),
      });
      localValues.replace(localScope(id), variables);
      imported += 1;
    }
    await workspace.loadOpenApiProvider(props.serviceId);
    environmentTransferMessage.value = t("openapi.environmentsImported", { count: imported });
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.environmentsImportInvalid");
  } finally {
    saving.value = false;
  }
}

function editEnvironment(environment: OpenApiEnvironment) {
  activeEnvironmentId.value = environment.id;
  environmentDraft.value = toDraft(environment, localValues.values(localScope(environment.id)));
  error.value = "";
}

function newEnvironment() {
  activeEnvironmentId.value = null;
  environmentDraft.value = { id: createId("openapi-env"), name: "", slug: "", requestMode: "inherit", baseUrl: "", servers: [], variables: [] };
  error.value = "";
}

async function saveSettings() {
  saving.value = true;
  error.value = "";
  settingsMessage.value = "";
  try {
    await workspace.updateOpenApiSettings(props.serviceId, settings.value);
    settingsMessage.value = t("openapi.settingsSaved");
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.saveFailed");
  } finally {
    saving.value = false;
  }
}

async function saveEnvironment(close: boolean) {
  if (!environmentDraft.value) return;
  saving.value = true;
  error.value = "";
  try {
    const draft = environmentDraft.value;
    const input = environmentInput(draft);
    const saved = draft.slug ? await workspace.updateOpenApiEnvironment(props.serviceId, draft.id, { ...input, slug: draft.slug }) : await workspace.createOpenApiEnvironment(props.serviceId, { id: draft.id, slug: draft.id, ...input });
    localValues.replace(
      localScope(draft.id),
      draft.variables.map((row) => ({ ...row, key: row.key.trim() })),
    );
    if (close) environmentDraft.value = null;
    else if (saved) environmentDraft.value = toDraft(saved, localValues.values(localScope(draft.id)));
    await workspace.loadOpenApiProvider(props.serviceId);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.saveFailed");
  } finally {
    saving.value = false;
  }
}

async function removeEnvironment() {
  const environment = environmentToDelete.value;
  if (!environment) return;
  saving.value = true;
  error.value = "";
  try {
    await workspace.deleteOpenApiEnvironment(props.serviceId, environment.id);
    environmentToDelete.value = null;
    if (activeEnvironmentId.value === environment.id) {
      activeEnvironmentId.value = null;
      environmentDraft.value = null;
    }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("openapi.saveFailed");
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="provider-panel min-w-0 space-y-6" :aria-label="t('openapi.providerOverview')">
    <header class="provider-hero">
      <div class="flex min-w-0 items-start gap-3">
        <span class="provider-icon"><Braces class="h-5 w-5" aria-hidden="true" /></span>
        <div class="min-w-0">
          <span class="eyebrow">{{ t("specification.kinds.openapi") }}</span>
          <h2>{{ t("openapi.providerOverview") }}</h2>
          <p>{{ t("openapi.providerDescription") }}</p>
        </div>
      </div>
      <div class="provider-stats" aria-label="OpenAPI statistics">
        <div>
          <strong>{{ provider.specificationCount }}</strong
          ><span>{{ t("openapi.specifications") }}</span>
        </div>
        <div>
          <strong>{{ provider.operationCount }}</strong
          ><span>{{ t("openapi.operations") }}</span>
        </div>
        <div>
          <strong>{{ provider.tagCount }}</strong
          ><span>{{ t("openapi.tags") }}</span>
        </div>
      </div>
    </header>

    <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>

    <section class="provider-section">
      <div class="section-heading">
        <div>
          <span class="eyebrow">{{ t("openapi.settings") }}</span>
          <h3>{{ t("openapi.settings") }}</h3>
        </div>
        <Settings2 class="h-4 w-4 text-muted-foreground" aria-hidden="true" />
      </div>
      <div class="provider-settings-grid">
        <label
          ><span>{{ t("openapi.defaultEnvironment") }}</span
          ><AppSelect v-model="settings.defaultEnvironmentId" size="form" class="field-control" name="openapi-default-environment">
            <option :value="null">{{ t("openapi.noDefaultEnvironment") }}</option>
            <option v-for="environment in provider.environments" :key="environment.id" :value="environment.id">{{ environment.name }}</option>
          </AppSelect></label
        >
        <label
          ><span>{{ t("openapi.defaultServerUrl") }}</span
          ><input v-model="settings.defaultServerUrl" class="field-control" name="openapi-default-server" type="url" autocomplete="off" :placeholder="t('openapi.defaultServerUrlPlaceholder')"
        /></label>
        <label
          ><span>{{ t("openapi.schemaDisplay") }}</span
          ><AppSelect v-model="settings.schemaDisplay" size="form" class="field-control" name="openapi-schema-display">
            <option value="summary">{{ t("openapi.schemaSummary") }}</option>
            <option value="full">{{ t("openapi.schemaFull") }}</option>
          </AppSelect></label
        >
      </div>
      <div class="flex flex-wrap items-center gap-5 pt-3 text-sm">
        <label class="inline-flex items-center gap-2"><input v-model="settings.showOperationId" type="checkbox" />{{ t("openapi.showOperationId") }}</label>
      </div>
      <div class="mt-4 flex items-center gap-3">
        <Button size="sm" :disabled="saving" @click="saveSettings"><Save class="h-3.5 w-3.5" />{{ saving ? t("common.saving") : t("common.save") }}</Button
        ><span v-if="settingsMessage" class="text-xs text-emerald-600" role="status">{{ settingsMessage }}</span>
      </div>
    </section>

    <section class="provider-section">
      <div class="section-heading">
        <div>
          <span class="eyebrow">{{ t("openapi.environments") }}</span>
          <h3>
            {{ t("openapi.environments") }} <small>{{ provider.environments.length }}</small>
          </h3>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <input ref="environmentImportInput" class="sr-only" type="file" accept="application/json,.json" :aria-label="t('openapi.importEnvironments')" @change="importEnvironments" />
          <Button size="sm" variant="ghost" :disabled="saving" @click="openEnvironmentImport"><Upload class="h-3.5 w-3.5" />{{ t("openapi.importEnvironments") }}</Button>
          <Button size="sm" variant="ghost" :disabled="saving || !provider.environments.length" @click="exportEnvironments"><Download class="h-3.5 w-3.5" />{{ t("openapi.exportEnvironments") }}</Button>
          <Button size="sm" variant="outline" @click="newEnvironment"><Plus class="h-3.5 w-3.5" />{{ t("openapi.newEnvironment") }}</Button>
        </div>
      </div>
      <p v-if="environmentTransferMessage" class="text-xs text-emerald-600" role="status">{{ environmentTransferMessage }}</p>
      <div v-if="!provider.environments.length" class="section-empty">
        <KeyRound class="h-5 w-5" /><strong>{{ t("openapi.noEnvironments") }}</strong
        ><span>{{ t("openapi.noEnvironmentsDescription") }}</span>
      </div>
      <div v-else class="environment-list">
        <div v-for="environment in provider.environments" :key="environment.id" class="environment-entry">
          <button type="button" class="environment-row" :class="{ active: activeEnvironmentId === environment.id }" @click="editEnvironment(environment)">
            <span class="environment-dot" /><span class="min-w-0 flex-1 text-left"
              ><strong>{{ environment.name }}</strong
              ><small>{{ environment.baseUrl || t("openapi.noBaseUrl") }} · {{ environment.variables.length }} {{ t("openapi.variables") }}</small></span
            ><span v-if="settings.defaultEnvironmentId === environment.id" class="environment-default">{{ t("openapi.default") }}</span></button
          ><button type="button" class="icon-button" :aria-label="t('common.delete')" @click="environmentToDelete = environment"><Trash2 class="h-3.5 w-3.5" /></button>
        </div>
      </div>
      <div v-if="environmentDraft" class="environment-editor">
        <div class="section-heading">
          <div>
            <span class="eyebrow">{{ activeEnvironment ? t("openapi.editEnvironment") : t("openapi.newEnvironment") }}</span>
            <h4>{{ environmentDraft.name || t("openapi.untitledEnvironment") }}</h4>
          </div>
          <button class="icon-button" type="button" :aria-label="t('common.close')" @click="environmentDraft = null"><X class="h-3.5 w-3.5" /></button>
        </div>
        <EnvironmentEditor v-model="environmentDraft" :busy="saving" :error="error" @save="saveEnvironment" />
      </div>
    </section>
    <Dialog :open="!!environmentToDelete" @update:open="(open) => !open && (environmentToDelete = null)">
      <DialogContent>
        <DialogHeader
          ><DialogTitle>{{ t("openapi.deleteEnvironmentTitle") }}</DialogTitle></DialogHeader
        >
        <p class="text-sm text-muted-foreground">{{ t("openapi.confirmDeleteEnvironment", { name: environmentToDelete?.name ?? "" }) }}</p>
        <DialogFooter
          ><Button variant="outline" :disabled="saving" @click="environmentToDelete = null">{{ t("common.cancel") }}</Button
          ><Button variant="destructive" :disabled="saving" @click="removeEnvironment"><Trash2 class="h-3.5 w-3.5" />{{ saving ? t("common.saving") : t("common.delete") }}</Button></DialogFooter
        >
      </DialogContent>
    </Dialog>
  </div>
</template>
