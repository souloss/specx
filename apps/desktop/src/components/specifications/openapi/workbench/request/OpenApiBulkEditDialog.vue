<script setup lang="ts">
import { computed, ref, watch, useId } from "vue";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { formatWorkbenchBulkEdit, parseWorkbenchBulkEdit, type WorkbenchRow, type WorkbenchBulkEditFormat } from "@/lib/specifications/openapi/workbench/openapiWorkbench";
import { translate as t } from "@/i18n";
const props = defineProps<{ rows: WorkbenchRow[] }>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ apply: [rows: WorkbenchRow[]] }>();
const format = ref<WorkbenchBulkEditFormat>("comma");
const text = ref("");
const error = ref<number>();
const errorId = useId();
let metadata: WorkbenchRow[] = [];
function withMetadata(rows: WorkbenchRow[]) {
  const available = [...metadata];
  return rows.map((row) => {
    const index = available.findIndex((old) => old.name === row.name);
    const old = index < 0 ? undefined : available.splice(index, 1)[0];
    return { ...old, ...row };
  });
}
const hint = computed(() => t(format.value === "comma" ? "specification.openapiBulkCsvHint" : "specification.openapiBulkColonHint"));
watch(open, (value) => {
  if (value) {
    metadata = props.rows.map((row) => ({ ...row }));
    text.value = formatWorkbenchBulkEdit(props.rows, format.value);
    error.value = undefined;
  }
});
function changeFormat(value: WorkbenchBulkEditFormat) {
  const parsed = parseWorkbenchBulkEdit(text.value, format.value);
  if (parsed.error) {
    error.value = parsed.error.line;
    return;
  }
  metadata = format.value === "colon" ? withMetadata(parsed.rows) : parsed.rows;
  format.value = value;
  text.value = formatWorkbenchBulkEdit(metadata, value);
  error.value = undefined;
}
function apply() {
  const parsed = parseWorkbenchBulkEdit(text.value, format.value);
  if (parsed.error) {
    error.value = parsed.error.line;
    return;
  }
  const rows = format.value === "colon" ? withMetadata(parsed.rows) : parsed.rows;
  emit("apply", rows);
  open.value = false;
}
</script>
<template>
  <Dialog v-model:open="open"
    ><DialogContent class="openapi-bulk-dialog openapi-workbench-surface">
      <DialogHeader
        ><DialogTitle>{{ t("specification.openapiBulkEdit") }}</DialogTitle
        ><DialogDescription class="sr-only">{{ hint }}</DialogDescription></DialogHeader
      >
      <div class="bulk-mode-switch" role="group" :aria-label="t('specification.openapiBulkEdit')">
        <button v-for="mode in ['comma', 'colon'] as const" :key="mode" type="button" :aria-pressed="format === mode" @click="changeFormat(mode)">{{ t(mode === "comma" ? "specification.openapiBulkComma" : "specification.openapiBulkColon") }}</button>
      </div>
      <p class="bulk-format-hint">{{ hint }}</p>
      <div class="bulk-editor">
        <textarea v-model="text" :aria-label="t('specification.openapiBulkEdit')" :aria-invalid="Boolean(error)" :aria-describedby="error ? errorId : undefined" spellcheck="false" autocapitalize="none" @input="error = undefined" />
        <p>{{ t(format === "comma" ? "specification.openapiBulkCsvHelp" : "specification.openapiBulkColonHelp") }}</p>
      </div>
      <p v-if="error" :id="errorId" role="alert" class="debug-bulk-edit-error">{{ t("specification.openapiBulkEditLineError", { line: error }) }}</p>
      <DialogFooter
        ><Button type="button" variant="outline" @click="open = false">{{ t("common.cancel") }}</Button
        ><Button type="button" @click="apply">{{ t("common.done") }}</Button></DialogFooter
      >
    </DialogContent></Dialog
  >
</template>
