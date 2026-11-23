<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogFooter, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import AppSelect from "@/components/ui/AppSelect.vue";
import { translate as t } from "@/i18n";
import { responseHasBody } from "@/lib/specifications/openapi/design/openapiResponseExamples";

const props = defineProps<{ usedStatuses: string[] }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ add: [response: { status: string; description: string; mediaType: string }] }>();

// Standard reason phrases provide an editable starting point for the OpenAPI description.
const statusOptions = [
  ["1XX", "Informational"],
  ["100", "Continue"],
  ["101", "Switching Protocols"],
  ["102", "Processing"],
  ["103", "Early Hints"],
  ["2XX", "Successful"],
  ["200", "OK"],
  ["201", "Created"],
  ["202", "Accepted"],
  ["203", "Non-Authoritative Information"],
  ["204", "No Content"],
  ["205", "Reset Content"],
  ["206", "Partial Content"],
  ["207", "Multi-Status"],
  ["208", "Already Reported"],
  ["226", "IM Used"],
  ["3XX", "Redirection"],
  ["300", "Multiple Choices"],
  ["301", "Moved Permanently"],
  ["302", "Found"],
  ["303", "See Other"],
  ["304", "Not Modified"],
  ["307", "Temporary Redirect"],
  ["308", "Permanent Redirect"],
  ["4XX", "Client Error"],
  ["400", "Bad Request"],
  ["401", "Unauthorized"],
  ["402", "Payment Required"],
  ["403", "Forbidden"],
  ["404", "Not Found"],
  ["405", "Method Not Allowed"],
  ["406", "Not Acceptable"],
  ["407", "Proxy Authentication Required"],
  ["408", "Request Timeout"],
  ["409", "Conflict"],
  ["410", "Gone"],
  ["411", "Length Required"],
  ["412", "Precondition Failed"],
  ["413", "Content Too Large"],
  ["414", "URI Too Long"],
  ["415", "Unsupported Media Type"],
  ["416", "Range Not Satisfiable"],
  ["417", "Expectation Failed"],
  ["418", "I'm a Teapot"],
  ["421", "Misdirected Request"],
  ["422", "Unprocessable Content"],
  ["423", "Locked"],
  ["424", "Failed Dependency"],
  ["425", "Too Early"],
  ["426", "Upgrade Required"],
  ["428", "Precondition Required"],
  ["429", "Too Many Requests"],
  ["431", "Request Header Fields Too Large"],
  ["451", "Unavailable For Legal Reasons"],
  ["5XX", "Server Error"],
  ["500", "Internal Server Error"],
  ["501", "Not Implemented"],
  ["502", "Bad Gateway"],
  ["503", "Service Unavailable"],
  ["504", "Gateway Timeout"],
  ["505", "HTTP Version Not Supported"],
  ["506", "Variant Also Negotiates"],
  ["507", "Insufficient Storage"],
  ["508", "Loop Detected"],
  ["510", "Not Extended"],
  ["511", "Network Authentication Required"],
  ["default", "Default"],
] as const;
const defaultStatusPriority = ["200", "201", "202", "204", "400", "404", "500", "default"];
const status = ref("");
const description = ref("");
const mediaType = ref("application/json");
const available = computed(() => statusOptions.filter(([code]) => !props.usedStatuses.includes(code)));

watch(open, (value) => {
  if (!value) return;
  const first = defaultStatusPriority.map((code) => available.value.find(([availableCode]) => availableCode === code)).find((option) => option !== undefined) ?? available.value.find(([code]) => /^\d{3}$/u.test(code)) ?? available.value[0];
  status.value = first?.[0] ?? "";
  description.value = first?.[1] ?? "";
  mediaType.value = "application/json";
});

function changeStatus(value: string) {
  status.value = value;
  description.value = statusOptions.find(([code]) => code === value)?.[1] ?? "";
}

function submit() {
  if (!status.value || props.usedStatuses.includes(status.value)) return;
  emit("add", { status: status.value, description: description.value.trim(), mediaType: responseHasBody(status.value) ? mediaType.value : "" });
  open.value = false;
}
</script>

<template>
  <Dialog :open="open" @update:open="open = $event">
    <DialogScrollContent class="grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 sm:max-w-lg">
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle>{{ t("specification.openapiDesignAddResponse") }}</DialogTitle>
      </DialogHeader>
      <form id="openapi-add-response-form" class="min-h-0 space-y-4 overflow-y-auto overscroll-contain px-5 py-4" @submit.prevent="submit">
        <label class="openapi-design-field">
          <span>{{ t("specification.openapiDesignResponseStatus") }}</span>
          <AppSelect :model-value="status" size="form" name="openapi-new-response-status" class="w-full" :aria-label="t('specification.openapiDesignResponseStatus')" @update:model-value="changeStatus">
            <option v-for="[code, name] in available" :key="code" :value="code">{{ code }} {{ name }}</option>
          </AppSelect>
        </label>
        <label class="openapi-design-field">
          <span>{{ t("specification.openapiDesignResponseName") }}</span>
          <input v-model="description" name="openapi-new-response-name" class="field-control" autocomplete="off" />
        </label>
        <label v-if="responseHasBody(status)" class="openapi-design-field">
          <span>{{ t("specification.openapiDesignMediaType") }}</span>
          <AppSelect v-model="mediaType" size="form" name="openapi-new-response-media-type" class="w-full" :aria-label="t('specification.openapiDesignMediaType')">
            <option value="application/json">application/json</option>
            <option value="application/problem+json">application/problem+json</option>
            <option value="application/xml">application/xml</option>
            <option value="text/xml">text/xml</option>
            <option value="application/yaml">application/yaml</option>
            <option value="text/csv">text/csv</option>
            <option value="text/plain">text/plain</option>
            <option value="text/html">text/html</option>
            <option value="application/pdf">application/pdf</option>
            <option value="image/png">image/png</option>
            <option value="image/jpeg">image/jpeg</option>
            <option value="application/octet-stream">application/octet-stream</option>
            <option value="">{{ t("specification.openapiDesignNoContentFormat") }}</option>
          </AppSelect>
        </label>
        <p v-else class="text-sm text-muted-foreground">{{ t("specification.openapiDesignResponseNoBody") }}</p>
      </form>
      <DialogFooter class="m-0 rounded-b-lg border-t border-border px-5 py-4">
        <Button variant="outline" @click="open = false">{{ t("common.cancel") }}</Button>
        <Button type="submit" form="openapi-add-response-form" :disabled="!status">{{ t("specification.openapiDesignAddResponse") }}</Button>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
