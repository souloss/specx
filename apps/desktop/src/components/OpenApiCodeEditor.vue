<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup, EditorView } from "codemirror";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { Compartment, EditorState } from "@codemirror/state";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
import { formatOpenApiContent } from "@/lib/openapi";

const props = defineProps<{ modelValue: string; label: string; format?: "json" | "yaml"; compact?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: string]; "update:format": [format: "json" | "yaml"] }>();
const t = translate;
const editorHost = ref<HTMLElement>();
const formatError = ref("");
const selectedFormat = computed(() => props.format ?? (props.modelValue.trimStart().startsWith("{") ? "json" : "yaml"));
const language = new Compartment();
let editor: EditorView | undefined;

function languageExtension() {
  return selectedFormat.value === "json" ? json() : yaml();
}

function setFormat(format: "json" | "yaml") {
  formatError.value = "";
  if (format === selectedFormat.value) return;
  try {
    const formatted = formatOpenApiContent(props.modelValue, format);
    emit("update:format", format);
    emit("update:modelValue", formatted);
  } catch {
    formatError.value = t("specification.openapiFormatInvalid");
  }
}

function formatContent() {
  formatError.value = "";
  try {
    emit("update:modelValue", formatOpenApiContent(props.modelValue, selectedFormat.value));
  } catch {
    formatError.value = t("specification.openapiFormatInvalid");
  }
}

onMounted(() => {
  if (!editorHost.value) return;
  editor = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        language.of(languageExtension()),
        EditorView.contentAttributes.of({ "aria-label": props.label, spellcheck: "false" }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) emit("update:modelValue", update.state.doc.toString());
        }),
        EditorView.theme({
          "&": { height: "100%", color: "var(--foreground)", backgroundColor: "transparent" },
          ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono, ui-monospace, monospace)" },
          ".cm-gutters": { backgroundColor: "transparent", borderRight: "1px solid var(--border)", color: "var(--muted-foreground)" },
          ".cm-activeLineGutter, .cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 55%, transparent)" },
          ".cm-focused": { outline: "none" },
          ".cm-content": { minHeight: props.compact ? "14rem" : "24rem", padding: "0.85rem 0" },
          ".cm-line": { paddingLeft: "0.8rem" },
          ".cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 28%, transparent) !important" },
        }),
      ],
    }),
    parent: editorHost.value,
  });
});

watch(
  () => props.modelValue,
  (value) => {
    if (!editor || value === editor.state.doc.toString()) return;
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } });
  },
);

watch(selectedFormat, () => editor?.dispatch({ effects: language.reconfigure(languageExtension()) }));
onBeforeUnmount(() => editor?.destroy());
</script>

<template>
  <div class="flex min-h-0 flex-col overflow-hidden rounded-md border border-border bg-background" :class="compact ? 'h-64' : 'h-full min-h-[22rem]'">
    <div class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border bg-muted/20 px-2 py-1.5">
      <div class="flex items-center gap-1" role="group" :aria-label="t('specification.openapiFormat')">
        <Button size="xs" :variant="selectedFormat === 'json' ? 'secondary' : 'ghost'" :aria-pressed="selectedFormat === 'json'" @click="setFormat('json')">JSON</Button>
        <Button size="xs" :variant="selectedFormat === 'yaml' ? 'secondary' : 'ghost'" :aria-pressed="selectedFormat === 'yaml'" @click="setFormat('yaml')">YAML</Button>
      </div>
      <Button size="xs" variant="outline" @click="formatContent">{{ t("specification.formatDocument") }}</Button>
    </div>
    <div ref="editorHost" class="min-h-0 flex-1 overflow-hidden text-xs sm:text-sm" />
    <p v-if="formatError" class="border-t border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive" role="alert">{{ formatError }}</p>
  </div>
</template>
