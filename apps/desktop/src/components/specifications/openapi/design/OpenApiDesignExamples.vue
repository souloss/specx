<script setup lang="ts">
import { Plus, Sparkles, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { translate as t } from "@/i18n";

type ExampleRow = { name: string; body: string | null };
defineProps<{ mediaType: string; rows: ExampleRow[]; editable: boolean; canGenerate: boolean; request?: boolean }>();
const emit = defineEmits<{ add: []; generate: []; edit: [row: ExampleRow]; remove: [name: string] }>();
</script>

<template>
  <section class="openapi-design-example-list">
    <header class="openapi-design-schema-heading">
      <strong>{{ t(request ? "specification.openapiDesignRequestExamples" : "specification.openapiResponseExample.heading") }}</strong>
      <code>{{ mediaType }}</code>
      <div v-if="editable || $slots.actions" class="ml-auto flex flex-wrap gap-1">
        <slot name="actions" />
        <Button v-if="editable && canGenerate" size="xs" variant="ghost" @click="emit('generate')"><Sparkles class="size-3" />{{ t("specification.openapiDesignGenerateExample") }}</Button>
        <Button v-if="editable" size="xs" variant="ghost" @click="emit('add')"><Plus class="size-3" />{{ t("specification.openapiResponseExample.add") }}</Button>
      </div>
    </header>
    <div v-for="row in rows" :key="row.name" class="flex min-w-0 items-center gap-2 rounded border border-border px-2 py-1">
      <code class="min-w-0 flex-1 truncate" :title="row.name">{{ row.name || t("specification.openapiResponseExample.default") }}</code>
      <span v-if="row.body === null" class="text-xs text-muted-foreground">{{ t("specification.openapiResponseExample.reference") }}</span>
      <template v-else>
        <Button size="xs" variant="ghost" @click="emit('edit', row)">{{ t(editable ? "common.edit" : "specification.openapiDesignViewExample") }}</Button>
        <button v-if="editable" type="button" class="icon-button" :aria-label="t('specification.openapiResponseExample.remove', { name: row.name || t('specification.openapiResponseExample.default') })" @click="emit('remove', row.name)"><Trash2 class="size-3.5" /></button>
      </template>
    </div>
  </section>
</template>
