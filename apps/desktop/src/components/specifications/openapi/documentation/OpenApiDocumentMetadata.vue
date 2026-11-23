<script setup lang="ts">
import { computed } from "vue";
import { ExternalLink, Mail } from "@lucide/vue";
import { isRecord, type OpenApiObject } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";

const props = defineProps<{ info?: unknown; externalDocs?: unknown }>();
const info = computed(() => (isRecord(props.info) ? props.info : {}));
const contact = computed(() => (isRecord(info.value.contact) ? info.value.contact : {}));
const license = computed(() => (isRecord(info.value.license) ? info.value.license : {}));
const external = computed(() => (isRecord(props.externalDocs) ? props.externalDocs : {}));
function text(object: OpenApiObject, key: string): string {
  return typeof object[key] === "string" ? object[key] : "";
}
function link(value: unknown): string | undefined {
  if (typeof value !== "string") return;
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) ? url.href : undefined;
  } catch {
    return;
  }
}
</script>

<template>
  <div v-if="Object.keys(contact).length || Object.keys(license).length || link(info.termsOfService) || link(external.url)" class="flex flex-wrap gap-3 text-xs">
    <div v-if="Object.keys(contact).length" class="min-w-0 rounded-lg border border-border bg-card px-3 py-2.5">
      <p class="mb-1 font-medium text-muted-foreground">{{ t("openapiPublication.contact") }}</p>
      <a v-if="link(contact.url)" :href="link(contact.url)" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 break-all text-primary underline-offset-4 hover:underline"
        >{{ text(contact, "name") || text(contact, "url") }}<ExternalLink class="size-3 shrink-0" aria-hidden="true"
      /></a>
      <p v-else-if="contact.name" class="break-words">{{ text(contact, "name") }}</p>
      <a v-if="typeof contact.email === 'string' && /^[^\s@]+@[^\s@]+$/.test(contact.email)" :href="'mailto:' + encodeURIComponent(contact.email)" class="mt-1 flex items-center gap-1 break-all text-primary underline-offset-4 hover:underline"
        ><Mail class="size-3 shrink-0" aria-hidden="true" />{{ contact.email }}</a
      >
    </div>
    <div v-if="license.name || license.identifier" class="min-w-0 rounded-lg border border-border bg-card px-3 py-2.5">
      <p class="mb-1 font-medium text-muted-foreground">{{ t("openapiPublication.license") }}</p>
      <a v-if="link(license.url)" :href="link(license.url)" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 break-all text-primary underline-offset-4 hover:underline"
        >{{ text(license, "name") || text(license, "identifier") }}<ExternalLink class="size-3 shrink-0" aria-hidden="true"
      /></a>
      <p v-else class="break-words">{{ text(license, "name") || text(license, "identifier") }}</p>
    </div>
    <div v-if="link(info.termsOfService) || link(external.url)" class="flex min-w-0 flex-wrap items-center gap-3 rounded-lg border border-border bg-card px-3 py-2.5">
      <a v-if="link(info.termsOfService)" :href="link(info.termsOfService)" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 break-all text-primary underline-offset-4 hover:underline"
        >{{ t("openapiPublication.termsOfService") }}<ExternalLink class="size-3 shrink-0" aria-hidden="true"
      /></a>
      <a v-if="link(external.url)" :href="link(external.url)" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1 break-all text-primary underline-offset-4 hover:underline"
        >{{ text(external, "description") || t("openapiPublication.externalDocs") }}<ExternalLink class="size-3 shrink-0" aria-hidden="true"
      /></a>
    </div>
  </div>
</template>
