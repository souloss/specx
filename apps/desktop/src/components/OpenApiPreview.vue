<script setup lang="ts">
import { computed } from "vue";
import { Braces, ExternalLink } from "@lucide/vue";
import { translate } from "@/i18n";
import { getOpenApiOperations, isRecord, parseOpenApiContent } from "@/lib/openapi";

const props = defineProps<{ content: string; focusOperationKey?: string }>();
const t = translate;
const document = computed(() => parseOpenApiContent(props.content));
const info = computed(() => (isRecord(document.value?.info) ? document.value.info : null));
const servers = computed(() => (Array.isArray(document.value?.servers) ? document.value.servers.filter((server): server is Record<string, unknown> => isRecord(server) && typeof server.url === "string") : []));
const operations = computed(() => getOpenApiOperations(props.content));
const focusedEntry = computed(() => operations.value.find((entry) => entry.identityKey === props.focusOperationKey));
const focusedPathItem = computed<Record<string, unknown> | null>(() => {
  const paths = document.value?.paths;
  if (!focusedEntry.value || !isRecord(paths)) return null;
  const pathItem = paths[focusedEntry.value.path];
  return isRecord(pathItem) ? pathItem : null;
});
const parameters = computed(() => {
  if (!focusedEntry.value) return [];
  const pathParameters = Array.isArray(focusedPathItem.value?.parameters) ? focusedPathItem.value.parameters : [];
  const operationParameters = Array.isArray(focusedEntry.value.operation.parameters) ? focusedEntry.value.operation.parameters : [];
  const merged = new Map<string, Record<string, unknown>>();
  for (const parameter of [...pathParameters, ...operationParameters]) {
    if (!isRecord(parameter)) continue;
    const key = `${String(parameter.in ?? "")}:${String(parameter.name ?? "")}`;
    merged.set(key, parameter);
  }
  return [...merged.values()];
});
const requestBody = computed(() => (isRecord(focusedEntry.value?.operation.requestBody) ? focusedEntry.value.operation.requestBody : null));
const responses = computed(() => (isRecord(focusedEntry.value?.operation.responses) ? Object.entries(focusedEntry.value.operation.responses) : []));
const security = computed(() => {
  const configured = focusedEntry.value?.operation.security ?? document.value?.security;
  return Array.isArray(configured) ? configured.filter(isRecord) : [];
});

function record(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

function schemaType(value: unknown): string {
  const schema = record(value);
  if (!schema) return t("specification.openapiUnspecified");
  if (typeof schema.$ref === "string") return schema.$ref;
  const type = Array.isArray(schema.type) ? schema.type.join(" | ") : typeof schema.type === "string" ? schema.type : "object";
  return typeof schema.format === "string" ? `${type} (${schema.format})` : type;
}

function pretty(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

function methodClass(method: string): string {
  const classes: Record<string, string> = {
    GET: "bg-emerald-600 text-white",
    POST: "bg-blue-600 text-white",
    PUT: "bg-amber-500 text-black",
    PATCH: "bg-orange-600 text-white",
    DELETE: "bg-rose-600 text-white",
    OPTIONS: "bg-slate-600 text-white",
    HEAD: "bg-cyan-700 text-white",
    TRACE: "bg-violet-700 text-white",
  };
  return classes[method] ?? "bg-muted text-foreground";
}
</script>

<template>
  <article v-if="props.focusOperationKey" class="min-w-0" :aria-label="t('specification.openapiOperation')">
    <template v-if="focusedEntry">
      <header class="border-b border-border pb-5">
        <div class="flex flex-wrap items-center gap-3">
          <span class="rounded px-2.5 py-1 font-mono text-xs font-bold" :class="methodClass(focusedEntry.method.toUpperCase())">{{ focusedEntry.method.toUpperCase() }}</span>
          <code class="min-w-0 break-all font-mono text-xl font-semibold">{{ focusedEntry.path }}</code>
        </div>
        <h3 v-if="focusedEntry.operation.summary" class="mt-4 text-lg font-semibold text-foreground">{{ focusedEntry.operation.summary }}</h3>
        <p v-if="focusedEntry.operation.description" class="mt-2 max-w-4xl whitespace-pre-wrap text-sm leading-6 text-muted-foreground">{{ focusedEntry.operation.description }}</p>
        <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span v-if="focusedEntry.operation.operationId"
            ><span class="font-medium text-foreground">operationId</span> <code>{{ focusedEntry.operation.operationId }}</code></span
          >
          <span v-for="tag in focusedEntry.tags" :key="tag" class="rounded-full border border-border px-2 py-0.5">{{ tag }}</span>
        </div>
      </header>

      <section v-if="parameters.length" class="border-b border-border py-5">
        <h4 class="mb-3 text-sm font-semibold">{{ t("specification.openapiParameters") }}</h4>
        <div class="overflow-x-auto">
          <table class="w-full min-w-[34rem] border-collapse text-left text-sm">
            <thead>
              <tr class="border-b border-border text-xs text-muted-foreground">
                <th class="px-3 py-2 font-medium">{{ t("specification.openapiName") }}</th>
                <th class="px-3 py-2 font-medium">{{ t("specification.openapiLocation") }}</th>
                <th class="px-3 py-2 font-medium">{{ t("specification.openapiType") }}</th>
                <th class="px-3 py-2 font-medium">{{ t("specification.openapiRequired") }}</th>
                <th class="px-3 py-2 font-medium">{{ t("specification.openapiDescription") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="parameter in parameters" :key="`${parameter.in}:${parameter.name}`" class="border-b border-border/70 align-top last:border-0">
                <td class="px-3 py-3 font-mono text-xs">{{ parameter.name }}</td>
                <td class="px-3 py-3 text-xs text-muted-foreground">{{ parameter.in }}</td>
                <td class="px-3 py-3 font-mono text-xs">{{ schemaType(parameter.schema) }}</td>
                <td class="px-3 py-3 text-xs">{{ parameter.required ? t("common.yes") : t("common.no") }}</td>
                <td class="max-w-md px-3 py-3 text-xs leading-5 text-muted-foreground">{{ parameter.description || "-" }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section v-if="requestBody" class="border-b border-border py-5">
        <div class="mb-3 flex flex-wrap items-baseline gap-2">
          <h4 class="text-sm font-semibold">{{ t("specification.openapiRequestBody") }}</h4>
          <span v-if="requestBody.required" class="text-xs text-rose-600">{{ t("specification.openapiRequired") }}</span>
        </div>
        <p v-if="requestBody.description" class="mb-3 text-sm text-muted-foreground">{{ requestBody.description }}</p>
        <div v-if="isRecord(requestBody.content)" class="space-y-4">
          <div v-for="[mediaType, media] in Object.entries(requestBody.content)" :key="mediaType" class="min-w-0">
            <div class="mb-2 font-mono text-xs text-muted-foreground">{{ mediaType }}</div>
            <pre v-if="isRecord(media) && media.schema" class="max-h-80 overflow-auto rounded-md border border-border bg-muted/20 p-3 text-xs leading-5"><code>{{ pretty(media.schema) }}</code></pre>
          </div>
        </div>
      </section>

      <section v-if="responses.length" class="border-b border-border py-5">
        <h4 class="mb-3 text-sm font-semibold">{{ t("specification.openapiResponses") }}</h4>
        <div class="divide-y divide-border">
          <div v-for="[status, response] in responses" :key="status" class="grid gap-2 py-3 sm:grid-cols-[5rem_minmax(0,1fr)]">
            <code class="font-mono text-sm font-semibold" :class="status.startsWith('2') ? 'text-emerald-700 dark:text-emerald-400' : status.startsWith('4') || status.startsWith('5') ? 'text-rose-700 dark:text-rose-400' : 'text-foreground'">{{ status }}</code>
            <div class="min-w-0">
              <p class="text-sm">{{ record(response)?.description || t("specification.openapiUnspecified") }}</p>
              <div v-if="record(response) && isRecord(record(response)?.content)" class="mt-3 space-y-3">
                <div v-for="[mediaType, media] in Object.entries(record(response)!.content as Record<string, unknown>)" :key="mediaType">
                  <div class="mb-1 font-mono text-xs text-muted-foreground">{{ mediaType }}</div>
                  <pre v-if="isRecord(media) && media.schema" class="max-h-80 overflow-auto rounded-md border border-border bg-muted/20 p-3 text-xs leading-5"><code>{{ pretty(media.schema) }}</code></pre>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section v-if="security.length" class="py-5">
        <h4 class="mb-3 text-sm font-semibold">{{ t("specification.openapiSecurity") }}</h4>
        <div class="flex flex-wrap gap-2">
          <code v-for="(requirement, index) in security" :key="index" class="rounded border border-border bg-muted/20 px-2 py-1 text-xs">{{ Object.keys(requirement).join(", ") || t("specification.openapiNoAuth") }}</code>
        </div>
      </section>
      <p v-if="!parameters.length && !requestBody && !responses.length && !security.length && !focusedEntry.operation.summary && !focusedEntry.operation.description" class="py-8 text-sm text-muted-foreground">{{ t("specification.openapiOperationEmpty") }}</p>
    </template>
    <div v-else class="border-l-2 border-amber-500 py-2 pl-4 text-sm text-muted-foreground" role="status">{{ t("specification.openapiOperationRemoved") }}</div>
  </article>

  <article v-else-if="document" class="min-w-0" :aria-label="t('specification.openapiPreview')">
    <header class="flex flex-wrap items-end justify-between gap-4 border-b border-border pb-5">
      <div class="min-w-0">
        <span class="eyebrow">OpenAPI {{ document.openapi }}</span>
        <h3 class="mt-1 break-words text-xl font-semibold">{{ info?.title || "OpenAPI" }}</h3>
        <p v-if="info?.description" class="mt-2 max-w-3xl whitespace-pre-wrap text-sm leading-6 text-muted-foreground">{{ info.description }}</p>
      </div>
      <div class="flex shrink-0 items-baseline gap-2 text-sm">
        <strong class="font-mono tabular-nums">{{ operations.length }}</strong
        ><span class="text-muted-foreground">{{ t("specification.openapiOperations") }}</span
        ><span v-if="info?.version" class="ml-2 border-l border-border pl-3 font-mono text-xs text-muted-foreground">{{ info.version }}</span>
      </div>
    </header>
    <section v-if="servers.length" class="border-b border-border py-4">
      <h4 class="mb-2 text-xs font-semibold uppercase text-muted-foreground">{{ t("specification.openapiServers") }}</h4>
      <div class="flex flex-wrap gap-x-5 gap-y-2">
        <a v-for="server in servers" :key="String(server.url)" class="inline-flex max-w-full items-center gap-1.5 font-mono text-xs text-primary hover:underline" :href="String(server.url)" target="_blank" rel="noreferrer"
          ><ExternalLink class="h-3 w-3 shrink-0" aria-hidden="true" /><span class="truncate">{{ server.url }}</span></a
        >
      </div>
    </section>
    <section class="py-4">
      <div class="mb-2 flex items-center justify-between">
        <h4 class="text-sm font-semibold">{{ t("specification.openapiOperations") }}</h4>
        <span class="text-xs text-muted-foreground">{{ t("specification.openapiMethod") }} · {{ t("specification.openapiPath") }}</span>
      </div>
      <div v-if="operations.length" class="divide-y divide-border border-y border-border">
        <div v-for="operation in operations" :key="operation.identityKey" class="grid min-w-0 grid-cols-[4.5rem_minmax(0,1fr)] gap-3 py-3">
          <span class="h-fit rounded px-2 py-1 text-center font-mono text-[10px] font-bold" :class="methodClass(operation.method.toUpperCase())">{{ operation.method.toUpperCase() }}</span>
          <div class="min-w-0">
            <code class="block break-all text-sm">{{ operation.path }}</code>
            <p v-if="operation.operation.summary || operation.operation.operationId" class="mt-1 text-xs text-muted-foreground">{{ operation.operation.summary || operation.operation.operationId }}</p>
            <div class="mt-1 flex flex-wrap gap-1.5">
              <span v-for="tag in operation.tags" :key="tag" class="text-[10px] text-muted-foreground">{{ tag }}</span>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="flex items-center gap-2 py-8 text-sm text-muted-foreground"><Braces class="h-4 w-4" aria-hidden="true" />{{ t("specification.openapiNoOperations") }}</div>
    </section>
  </article>
  <pre v-else class="max-h-[32rem] overflow-auto rounded-md border border-border bg-muted/10 p-4 font-mono text-xs leading-relaxed" :aria-label="t('specification.openapiRawFallback')">{{ content }}</pre>
</template>
