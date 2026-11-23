<script setup lang="ts">
import { computed } from "vue";
import { LockKeyhole, ShieldCheck } from "@lucide/vue";
import MarkdownPreview from "../../markdown/MarkdownPreview.vue";
import { isRecord, resolveOpenApiRef, type OpenApiObject, type OpenApiOperationEntry } from "@/lib/specifications/openapi/contract/openapi";
import { translate as t } from "@/i18n";
const props = defineProps<{ document: OpenApiObject | null; entry: OpenApiOperationEntry }>();
const requirements = computed(() => {
  const value = props.entry.operation.security ?? props.document?.security;
  return Array.isArray(value) ? value.filter(isRecord) : [];
});
const schemes = computed(() => (isRecord(props.document?.components) && isRecord(props.document.components.securitySchemes) ? props.document.components.securitySchemes : {}));
function scheme(name: string) {
  return resolveOpenApiRef(props.document, schemes.value[name]);
}
function credentialExample(value: OpenApiObject | null) {
  if (value?.type === "apiKey") return `${String(value.name)}: YOUR_API_KEY`;
  if (value?.type === "http" && value.scheme === "basic") return "Authorization: Basic BASE64_USERNAME_PASSWORD";
  if (value?.type === "http" || value?.type === "oauth2" || value?.type === "openIdConnect") return `Authorization: ${value.scheme === "bearer" || value.type !== "http" ? "Bearer" : String(value.scheme)} YOUR_ACCESS_TOKEN`;
  return "";
}
function flows(value: OpenApiObject | null) {
  return isRecord(value?.flows) ? Object.entries(value.flows).filter((entry): entry is [string, OpenApiObject] => isRecord(entry[1])) : [];
}
</script>

<template>
  <section class="openapi-security-document">
    <h4 class="mb-3 flex items-center gap-2 text-base font-semibold"><LockKeyhole class="size-4 text-primary" aria-hidden="true" />{{ t("specification.openapiSecurity") }}</h4>
    <div v-if="!requirements.length" class="flex items-center gap-2 rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground"><ShieldCheck class="size-4 shrink-0" aria-hidden="true" />{{ t("openapiPublication.authAnonymous") }}</div>
    <template v-for="(requirement, index) in requirements" :key="index">
      <div v-if="index" class="py-2 text-center text-[11px] font-medium text-muted-foreground">{{ t("openapiPublication.authOr") }}</div>
      <div class="space-y-4 rounded-xl border border-border bg-card p-4">
        <p v-if="!Object.keys(requirement).length" class="text-sm text-muted-foreground">{{ t("openapiPublication.authAnonymous") }}</p>
        <p v-if="Object.keys(requirement).length > 1" class="text-xs text-muted-foreground">{{ t("openapiPublication.authAllRequired") }}</p>
        <section v-for="(scopes, name) in requirement" :key="name" class="min-w-0 space-y-2">
          <template v-for="value in [scheme(String(name))]" :key="String(name)">
            <div class="flex flex-wrap items-center gap-2">
              <strong class="break-all text-sm" translate="no">{{ name }}</strong
              ><code v-if="value" class="rounded-md bg-primary/8 px-2 py-1 text-[11px] text-primary" translate="no"
                >{{ value.type }}<template v-if="value.scheme"> · {{ value.scheme }}</template></code
              >
            </div>
            <MarkdownPreview v-if="typeof value?.description === 'string'" :content="value.description" class="text-sm" />
            <p v-if="value?.in" class="text-xs text-muted-foreground">
              {{ t("openapiPublication.authLocation") }}: <code class="text-foreground" translate="no">{{ value.in }} · {{ value.name }}</code>
            </p>
            <pre v-if="credentialExample(value)" class="overflow-x-auto rounded-md bg-muted/40 px-3 py-2 text-xs" tabindex="0" translate="no">{{ credentialExample(value) }}</pre>
            <p v-if="Array.isArray(scopes) && scopes.length" class="break-words text-xs text-muted-foreground">
              {{ t("openapiPublication.authScopes") }}: <code class="text-foreground" translate="no">{{ scopes.join(", ") }}</code>
            </p>
            <p v-if="value?.openIdConnectUrl" class="break-all font-mono text-xs text-muted-foreground" translate="no">{{ value.openIdConnectUrl }}</p>
            <details v-if="flows(value).length" class="rounded-md border border-border px-3 py-2 text-xs">
              <summary class="cursor-pointer text-muted-foreground">{{ t("openapiPublication.oauthFlows") }}</summary>
              <dl v-for="[name, flow] in flows(value)" :key="name" class="mt-3 space-y-2">
                <dt class="font-mono font-medium" translate="no">{{ name }}</dt>
                <dd v-for="key in ['authorizationUrl', 'tokenUrl', 'refreshUrl']" v-show="flow[key]" :key="key" class="break-all font-mono text-muted-foreground" translate="no">{{ key }}: {{ flow[key] }}</dd>
                <dd v-if="isRecord(flow.scopes)" class="space-y-1">
                  <p v-for="(description, scope) in flow.scopes" :key="scope" class="break-words">
                    <code translate="no">{{ scope }}</code> · {{ description }}
                  </p>
                </dd>
              </dl>
            </details>
          </template>
        </section>
      </div>
    </template>
  </section>
</template>
