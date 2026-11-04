<!-- Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. -->
<script setup lang="ts">
import type { DialogCloseProps } from "reka-ui";
import { DialogClose } from "reka-ui";

const props = defineProps<DialogCloseProps>();
</script>

<template>
  <DialogClose data-slot="dialog-close" v-bind="props">
    <slot />
  </DialogClose>
</template>
