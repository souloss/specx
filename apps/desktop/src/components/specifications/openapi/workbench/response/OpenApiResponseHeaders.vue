<script setup lang="ts">
import { translate as t } from "@/i18n";
defineProps<{ headers: Array<[string, string]> }>();
</script>

<template>
  <div class="response-headers">
    <table v-if="headers.length" class="response-data-table">
      <thead>
        <tr>
          <th scope="col">{{ t("specification.openapiResponseDetails.name") }}</th>
          <th scope="col">{{ t("specification.openapiResponseDetails.value") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="([name, value], index) in headers" :key="`${name}-${index}`">
          <th scope="row">
            <code>{{ name }}</code>
          </th>
          <td>{{ value }}</td>
        </tr>
      </tbody>
    </table>
    <p v-else class="response-view-empty">{{ t("specification.openapiNoResponseHeaders") }}</p>
  </div>
</template>
