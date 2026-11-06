<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { FolderGit2, Link2, Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { createId } from "@/lib/core";
import { translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";
import type { CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSourceType } from "@/types/api";
import type { ServiceCodeBinding } from "@/types/domain";

const props = defineProps<{ serviceId: string }>();
const open = defineModel<boolean>("open", { required: true });
const workspace = useWorkspaceStore();
const t = translate;
const error = ref("");
const busy = ref(false);
const sourceId = ref("");
const sourceType = ref<CodeSourceType>("git");
const provider = ref("generic_git");
const displayName = ref("");
const canonicalKey = ref("");
const rootLocator = ref("");
const executionScope = ref<CodeExecutionScope>("server");
const role = ref<CodeBindingRole>("primary");
const entityKind = ref<CodeEntityKind>("directory");
const path = ref("");
const revisionKind = ref<CodeRevisionKind>("default_branch");
const revision = ref("");
const confirmBinding = ref<ServiceCodeBinding | null>(null);
const sources = computed(() => workspace.codeSources.filter((source) => !source.archived));
const bindings = computed(() => workspace.bindingsForService(props.serviceId));
const needsRevision = computed(() => ["branch", "tag", "commit"].includes(revisionKind.value));

watch(open, async (value) => {
  if (!value) return;
  error.value = "";
  sourceType.value = "git";
  provider.value = "generic_git";
  displayName.value = "";
  canonicalKey.value = "";
  rootLocator.value = "";
  executionScope.value = "server";
  role.value = bindings.value.some((binding) => binding.role === "primary") ? "additional" : "primary";
  entityKind.value = "directory";
  path.value = "";
  revisionKind.value = "default_branch";
  revision.value = "";
  try {
    await Promise.all([workspace.loadCodeSources(), workspace.loadCodeBindings(props.serviceId)]);
    sourceId.value = workspace.codeSources[0]?.id ?? "";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.codeBindingFailed");
  }
});

async function createSource() {
  if (busy.value || !displayName.value.trim() || !canonicalKey.value.trim() || !rootLocator.value.trim()) return;
  busy.value = true;
  error.value = "";
  try {
    const created = await workspace.createCodeSource({
      id: createId("source"),
      sourceType: sourceType.value,
      provider: provider.value.trim(),
      displayName: displayName.value.trim(),
      canonicalKey: canonicalKey.value.trim(),
      rootLocator: rootLocator.value.trim(),
      executionScope: executionScope.value,
    });
    sourceId.value = created.id;
    displayName.value = "";
    canonicalKey.value = "";
    rootLocator.value = "";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.codeSourceFailed");
  } finally {
    busy.value = false;
  }
}

async function saveBinding() {
  if (busy.value || !sourceId.value || (needsRevision.value && !revision.value.trim())) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.createCodeBinding(props.serviceId, { id: createId("binding"), sourceId: sourceId.value, role: role.value, entityKind: entityKind.value, path: path.value.trim(), revisionKind: revisionKind.value, revision: needsRevision.value ? revision.value.trim() : null });
    open.value = false;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.codeBindingFailed");
  } finally {
    busy.value = false;
  }
}

async function removeBinding() {
  if (!confirmBinding.value || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.deleteCodeBinding(confirmBinding.value);
    confirmBinding.value = null;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("service.codeBindingFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !busy && (open = value)">
    <DialogContent class="sm:max-w-3xl">
      <DialogHeader
        ><DialogTitle class="flex items-center gap-2"><Link2 class="h-4 w-4" />{{ t("service.codeBindings") }}</DialogTitle></DialogHeader
      >
      <div class="grid min-h-0 gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <section class="min-w-0 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <h3 class="text-sm font-medium">{{ t("service.addCodeBinding") }}</h3>
              <p class="text-xs text-muted-foreground">{{ t("service.addCodeBindingDescription") }}</p>
            </div>
            <FolderGit2 class="h-5 w-5 text-primary" />
          </div>
          <label class="settings-field"
            >{{ t("service.sourceType")
            }}<select v-model="sourceType" class="settings-input">
              <option value="git">{{ t("service.gitSource") }}</option>
              <option value="local_filesystem">{{ t("service.localSource") }}</option>
            </select></label
          >
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="settings-field">{{ t("service.provider") }}<input v-model="provider" class="settings-input" autocomplete="off" /></label
            ><label class="settings-field"
              >{{ t("service.executionScope")
              }}<select v-model="executionScope" class="settings-input">
                <option value="server">{{ t("service.serverScope") }}</option>
                <option value="desktop_host">{{ t("service.desktopScope") }}</option>
              </select></label
            >
          </div>
          <label class="settings-field">{{ t("service.rootLocator") }}<input v-model="rootLocator" class="settings-input" :placeholder="sourceType === 'git' ? 'https://github.com/org/repo.git' : '/workspace/root'" autocomplete="off" /></label>
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="settings-field">{{ t("service.sourceName") }}<input v-model="displayName" class="settings-input" autocomplete="off" /></label><label class="settings-field">{{ t("service.canonicalKey") }}<input v-model="canonicalKey" class="settings-input" autocomplete="off" /></label>
          </div>
          <Button type="button" variant="outline" size="sm" :disabled="busy || !displayName.trim() || !canonicalKey.trim() || !rootLocator.trim()" @click="createSource"><Plus class="h-3.5 w-3.5" />{{ t("service.createCodeSource") }}</Button>
          <label class="settings-field"
            >{{ t("service.codeBindings")
            }}<select v-model="sourceId" class="settings-input">
              <option value="" disabled>{{ t("service.codeSourceRequired") }}</option>
              <option v-for="source in sources" :key="source.id" :value="source.id">{{ source.displayName }} · {{ source.provider }}</option>
            </select></label
          >
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="settings-field"
              >{{ t("service.bindingRole")
              }}<select v-model="role" class="settings-input">
                <option value="primary">{{ t("service.primaryBinding") }}</option>
                <option value="additional">{{ t("service.additionalBinding") }}</option>
              </select></label
            ><label class="settings-field"
              >{{ t("service.entityKind")
              }}<select v-model="entityKind" class="settings-input">
                <option value="repository_root">{{ t("service.repositoryRoot") }}</option>
                <option value="directory">{{ t("service.codeDirectory") }}</option>
                <option value="file">{{ t("service.file") }}</option>
                <option value="pattern">{{ t("service.pattern") }}</option>
              </select></label
            >
          </div>
          <label class="settings-field">{{ t("service.codePath") }}<input v-model="path" class="settings-input" :placeholder="entityKind === 'repository_root' ? '/' : 'services/example'" autocomplete="off" /></label>
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="settings-field"
              >{{ t("service.revision")
              }}<select v-model="revisionKind" class="settings-input">
                <option value="default_branch">{{ t("service.defaultBranch") }}</option>
                <option value="branch">{{ t("service.branch") }}</option>
                <option value="tag">{{ t("service.tag") }}</option>
                <option value="commit">{{ t("service.commit") }}</option>
                <option value="working_tree">{{ t("service.workingTree") }}</option>
              </select></label
            ><label v-if="needsRevision" class="settings-field">{{ t("service.revisionValue") }}<input v-model="revision" class="settings-input" autocomplete="off" /></label>
          </div>
          <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
          <DialogFooter
            ><Button type="button" variant="outline" :disabled="busy" @click="open = false">{{ t("common.cancel") }}</Button
            ><Button type="button" :disabled="busy || !sourceId || (needsRevision && !revision.trim())" @click="saveBinding"><Link2 class="h-3.5 w-3.5" />{{ t("service.saveBinding") }}</Button></DialogFooter
          >
        </section>
        <section class="min-w-0 border-t border-border pt-4 lg:border-t-0 lg:border-l lg:pl-5 lg:pt-0">
          <h3 class="text-sm font-medium">{{ t("service.codeBindings") }}</h3>
          <p v-if="!bindings.length" class="mt-4 rounded-md border border-dashed border-border p-4 text-xs text-muted-foreground">{{ t("service.noCodeBindings") }}</p>
          <div v-else class="mt-3 space-y-2">
            <div v-for="binding in bindings" :key="binding.id" class="rounded-md border border-border p-3">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="flex items-center gap-2 text-sm font-medium">
                    <span class="truncate">{{ sources.find((source) => source.id === binding.sourceId)?.displayName ?? binding.sourceId }}</span
                    ><span class="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">{{ binding.role === "primary" ? t("service.primaryBinding") : t("service.additionalBinding") }}</span>
                  </div>
                  <p class="mt-1 truncate text-xs text-muted-foreground">
                    {{ binding.path || "/" }} · {{ binding.revisionKind }}<span v-if="binding.revision"> · {{ binding.revision }}</span>
                  </p>
                </div>
                <Button variant="ghost" size="icon-sm" :aria-label="t('service.removeBinding')" :disabled="busy" @click="confirmBinding = binding"><Trash2 class="h-3.5 w-3.5 text-destructive" /></Button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </DialogContent>
  </Dialog>
  <Dialog :open="Boolean(confirmBinding)" @update:open="(value) => !value && (confirmBinding = null)"
    ><DialogContent
      ><DialogHeader
        ><DialogTitle>{{ t("service.deleteCodeBindingTitle") }}</DialogTitle></DialogHeader
      >
      <p class="text-sm text-muted-foreground">{{ t("service.confirmDeleteCodeBinding", { path: confirmBinding?.path || "/" }) }}</p>
      <DialogFooter
        ><Button variant="outline" :disabled="busy" @click="confirmBinding = null">{{ t("common.cancel") }}</Button
        ><Button variant="destructive" :disabled="busy" @click="removeBinding"><Trash2 class="h-3.5 w-3.5" />{{ t("service.removeBinding") }}</Button></DialogFooter
      ></DialogContent
    ></Dialog
  >
</template>
