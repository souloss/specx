<script setup lang="ts">
import { translate as t } from "@/i18n";
const timeout = defineModel<number>("timeout", { required: true });
const redirects = defineModel<boolean>("redirects", { required: true });
const validateResponse = defineModel<boolean>("validateResponse", { required: true });
</script>
<template>
  <div class="debug-request-settings">
    <label class="debug-setting-row"
      ><span>{{ t("specification.openapiTimeoutSetting") }}</span>
      <div class="debug-setting-input">
        <input v-model.number="timeout" type="number" min="1" max="300" name="openapi-request-timeout" class="field-control" /><span>{{ t("specification.openapiSeconds") }}</span>
      </div></label
    >
    <label class="debug-setting-row"
      ><span>{{ t("specification.openapiFollowRedirects") }}</span
      ><input v-model="redirects" type="checkbox" role="switch" class="debug-switch" name="openapi-follow-redirects"
    /></label>
    <label class="debug-setting-row"
      ><span>{{ t("specification.openapiAutoValidateResponse") }}</span
      ><input v-model="validateResponse" type="checkbox" role="switch" class="debug-switch" name="openapi-auto-validate-response"
    /></label>
  </div>
</template>
