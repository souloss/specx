<!-- Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. -->
<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { DialogClose } from "reka-ui";
import { cn } from "@/lib/common/utils";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
const t = translate;

const props = withDefaults(
  defineProps<{
    class?: HTMLAttributes["class"];
    showCloseButton?: boolean;
  }>(),
  {
    showCloseButton: false,
  },
);
</script>

<template>
  <div data-slot="dialog-footer" :class="cn('bg-muted/50 -mx-4 -mb-4 rounded-b-xl border-t p-4 flex flex-col-reverse gap-2 sm:flex-row sm:flex-wrap sm:justify-end', props.class)">
    <slot />
    <DialogClose v-if="showCloseButton" as-child>
      <Button variant="outline">{{ t("common.close") }}</Button>
    </DialogClose>
  </div>
</template>
