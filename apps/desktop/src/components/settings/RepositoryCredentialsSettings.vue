<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { KeyRound, RefreshCw, Save } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { translate } from "@/i18n";
import { useWorkspaceStore } from "@/stores/workspace";

const workspace = useWorkspaceStore();
const t = translate;
const provider = ref("github");
const credential = ref("");
const busy = ref(false);
const loading = ref(true);
const error = ref("");
const saved = ref(false);

const providerOptions = computed(() => [
  { value: "github", label: t("settings.repositoryProviderGithub"), secretName: "github-token" },
  { value: "gitlab", label: t("settings.repositoryProviderGitlab"), secretName: "gitlab-token" },
  { value: "bitbucket", label: t("settings.repositoryProviderBitbucket"), secretName: "bitbucket-token" },
  { value: "generic_git", label: t("settings.repositoryProviderGeneric"), secretName: "generic-git-token" },
]);
const selectedProvider = computed(() => providerOptions.value.find((item) => item.value === provider.value) ?? providerOptions.value[0]!);
const integration = computed(() => workspace.integrations.find((item) => item.provider === provider.value));
const configured = computed(() => Boolean(integration.value?.secretId));

async function load() {
  loading.value = true;
  error.value = "";
  try {
    await workspace.loadRepositoryCredentials();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("settings.repositoryCredentialsLoadFailed");
  } finally {
    loading.value = false;
  }
}

watch(provider, () => {
  credential.value = "";
  saved.value = false;
  error.value = "";
});

onMounted(load);

async function save() {
  const value = credential.value.trim();
  if (!value || busy.value) return;
  busy.value = true;
  error.value = "";
  saved.value = false;
  try {
    await workspace.saveRepositoryCredential(provider.value, selectedProvider.value.label, selectedProvider.value.secretName, value);
    credential.value = "";
    saved.value = true;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("settings.repositoryCredentialsSaveFailed");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="space-y-5 py-2" aria-labelledby="repository-credentials-title">
    <div>
      <div class="flex items-center gap-2">
        <KeyRound class="h-4 w-4 text-primary" />
        <h2 id="repository-credentials-title" class="text-sm font-medium">{{ t("settings.repositoryCredentials") }}</h2>
      </div>
      <p class="mt-1 max-w-2xl text-xs text-muted-foreground">{{ t("settings.repositoryCredentialsDescription") }}</p>
    </div>
    <p v-if="error" role="alert" class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{{ error }}</p>
    <p v-if="loading" class="py-8 text-center text-sm text-muted-foreground">{{ t("common.loading") }}</p>
    <div v-else class="max-w-xl space-y-4">
      <div class="space-y-2">
        <Label for="repository-credential-provider">{{ t("settings.repositoryCredentialProvider") }}</Label>
        <Select v-model="provider">
          <SelectTrigger id="repository-credential-provider" class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem v-for="option in providerOptions" :key="option.value" :value="option.value">{{ option.label }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div class="space-y-2">
        <Label for="repository-credential-value">{{ configured ? t("settings.repositoryCredentialRotate") : t("settings.repositoryCredential") }}</Label>
        <Input id="repository-credential-value" v-model="credential" type="password" autocomplete="new-password" :placeholder="t('settings.repositoryCredentialPlaceholder')" :disabled="busy" @keydown.enter.prevent="save" />
        <p class="text-xs text-muted-foreground">{{ t("settings.repositoryCredentialHint") }}</p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <Button :disabled="busy || !credential.trim()" @click="save"><Save class="h-3.5 w-3.5" />{{ busy ? t("common.saving") : t("settings.saveRepositoryCredential") }}</Button>
        <Button variant="outline" :disabled="loading || busy" @click="load"><RefreshCw class="h-3.5 w-3.5" />{{ t("common.retry") }}</Button>
        <span v-if="saved" role="status" class="text-sm text-primary">{{ t("settings.repositoryCredentialSaved") }}</span>
      </div>
    </div>
  </section>
</template>
