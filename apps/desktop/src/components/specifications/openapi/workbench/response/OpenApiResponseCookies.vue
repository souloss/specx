<script setup lang="ts">
import { computed } from "vue";
import { parseResponseCookies } from "@/lib/specifications/openapi/workbench/response/openapiResponseCookies";
import { translate as t } from "@/i18n";
const props = defineProps<{ headers: Array<[string, string]>; url: string; browser: boolean }>();
const cookies = computed(() => parseResponseCookies(props.headers, props.url));
</script>
<template>
  <div v-if="browser" class="response-view-empty response-cookie-view">
    <strong>{{ t("specification.openapiResponseDetails.cookiesRestricted") }}</strong>
    <p>{{ t("specification.openapiResponseDetails.cookiesHelp") }}</p>
  </div>
  <div v-else class="response-cookie-table">
    <table>
      <thead>
        <tr>
          <th scope="col">Name</th>
          <th scope="col">Value</th>
          <th scope="col">Domain</th>
          <th scope="col">Path</th>
          <th scope="col">Expires / Max-Age</th>
          <th scope="col">HttpOnly</th>
          <th scope="col">Secure</th>
          <th scope="col">SameSite</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(cookie, index) in cookies" :key="index">
          <td>{{ cookie.name }}</td>
          <td>{{ cookie.value }}</td>
          <td>{{ cookie.domain }}</td>
          <td>{{ cookie.path }}</td>
          <td>{{ cookie.expires || t("requestAgent.sessionCookie") }}</td>
          <td>{{ cookie.httpOnly ? "✓" : "" }}</td>
          <td>{{ cookie.secure ? "✓" : "" }}</td>
          <td>{{ cookie.sameSite }}</td>
        </tr>
      </tbody>
    </table>
    <p v-if="!cookies.length" class="response-view-empty">{{ t("requestAgent.noCookies") }}</p>
  </div>
</template>
