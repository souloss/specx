<!-- Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. -->
<script setup lang="ts">
import type { SelectValueProps } from "reka-ui";
import { SelectValue } from "reka-ui";

const props = defineProps<SelectValueProps>();
</script>

<template>
  <SelectValue data-slot="select-value" v-bind="props">
    <slot />
  </SelectValue>
</template>
