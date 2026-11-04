<script setup lang="ts">
import { ref } from "vue";
import { Button } from "@/components/ui/button";
import { SpecxLogo } from "@/components/brand";
import { useWorkspaceStore } from "@/stores/workspace";
import { translate } from "@/i18n";
const workspace = useWorkspaceStore();
const t = translate;
const password = ref("");
const error = ref("");
const busy = ref(false);
async function login() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.login(password.value);
    password.value = "";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t("login.failed");
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <main class="auth-shell">
    <form class="auth-panel" @submit.prevent="login">
      <SpecxLogo :size="40" animated />
      <h1 class="text-lg font-medium">{{ t("login.title") }}</h1>
      <p class="text-sm text-muted-foreground">{{ t("login.description") }}</p>
      <label class="settings-field">{{ t("login.password") }}<input v-model="password" class="settings-input" type="password" autocomplete="current-password" autofocus required /></label>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <Button type="submit" :disabled="busy">{{ busy ? t("login.submitting") : t("login.submit") }}</Button>
    </form>
  </main>
</template>
