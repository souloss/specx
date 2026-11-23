<script setup lang="ts">
import { computed } from "vue";
import AppSelect from "@/components/ui/AppSelect.vue";
import { SNIPPET_CLIENTS, SNIPPET_FAMILIES, type SnippetLanguage } from "@/lib/specifications/openapi/snippets/openapiSnippetClients";
import { translate as t } from "@/i18n";
const language = defineModel<SnippetLanguage>({ required: true });
const family = computed({
  get: () => SNIPPET_CLIENTS.find((client) => client.id === language.value)!.family,
  set: (value: string) => {
    language.value = SNIPPET_CLIENTS.find((client) => client.family === value)!.id;
  },
});
const clients = computed(() => SNIPPET_CLIENTS.filter((client) => client.family === family.value));
</script>

<template>
  <div class="flex min-w-0 flex-wrap items-center gap-2">
    <AppSelect v-model="family" size="compact" name="openapi-snippet-language" :aria-label="t('specification.openapiSnippetLanguage')" class="text-xs">
      <option v-for="name in SNIPPET_FAMILIES" :key="name" :value="name">{{ name }}</option>
    </AppSelect>
    <AppSelect v-model="language" size="compact" name="openapi-snippet-client" :aria-label="t('openapiPublication.snippetClient')" class="text-xs">
      <option v-for="client in clients" :key="client.id" :value="client.id">{{ client.label }}</option>
    </AppSelect>
  </div>
</template>
