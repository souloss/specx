<script setup lang="ts">
import OpenApiDynamicValuePicker from "../workbench/variables/OpenApiDynamicValuePicker.vue";
import { openApiDynamicCompletion } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicCompletion";
import { dynamicExpressionRangeAt } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicVariables";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup, EditorView } from "codemirror";
import { json } from "@codemirror/lang-json";
import { xml } from "@codemirror/lang-xml";
import { html } from "@codemirror/lang-html";
import { yaml } from "@codemirror/lang-yaml";
import { Compartment, EditorState } from "@codemirror/state";
import { Button } from "@/components/ui/button";
import { translate } from "@/i18n";
import YAML from "yaml";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    mediaType?: string;
    label: string;
    name?: string;
    compact?: boolean;
  }>(),
  { mediaType: "", name: "openapi-request-body", compact: false },
);
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const t = translate;
const host = ref<HTMLElement>();
const error = ref("");
const language = new Compartment();
let editor: EditorView | undefined;

const format = computed<"json" | "yaml" | "text">(() => {
  const mediaType = props.mediaType.toLowerCase();
  if (mediaType.includes("yaml") || mediaType.includes("yml")) return "yaml";
  if (mediaType.includes("json") || props.modelValue.trimStart().startsWith("{") || props.modelValue.trimStart().startsWith("[")) return "json";
  return "text";
});

function languageExtension() {
  return props.mediaType.includes("html") ? html() : props.mediaType.includes("xml") ? xml() : format.value === "yaml" ? yaml() : format.value === "json" ? json() : [];
}

function formatBody() {
  error.value = "";
  if (!props.modelValue.trim()) return;
  try {
    const value: unknown = format.value === "yaml" ? YAML.parse(props.modelValue) : JSON.parse(props.modelValue);
    emit("update:modelValue", `${format.value === "yaml" ? YAML.stringify(value) : JSON.stringify(value, null, 2)}\n`);
  } catch {
    error.value = t("specification.openapiFormatInvalid");
  }
}

function insertDynamicValue(value: string) {
  if (!editor) return;
  const selection = editor.state.selection.main;
  const range = selection.empty ? (dynamicExpressionRangeAt(editor.state.doc.toString(), selection.from) ?? selection) : selection;
  editor.dispatch({ changes: { from: range.from, to: range.to, insert: value }, selection: { anchor: range.from + value.length }, userEvent: "input" });
  editor.focus();
}

onMounted(() => {
  if (!host.value) return;
  editor = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        EditorView.lineWrapping,
        openApiDynamicCompletion(),
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
          ".cm-content": { minHeight: props.compact ? "9rem" : "13rem", padding: "0.7rem 0" },
          ".cm-line": { paddingLeft: "0.7rem" },
          ".cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 28%, transparent) !important" },
        }),
      ],
    }),
    parent: host.value,
  });
});

watch(
  () => props.modelValue,
  (value) => {
    if (!editor || value === editor.state.doc.toString()) return;
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } });
  },
);
watch([format, () => props.mediaType], () => editor?.dispatch({ effects: language.reconfigure(languageExtension()) }));
onBeforeUnmount(() => editor?.destroy());
</script>

<template>
  <div class="openapi-payload-editor" :class="{ compact: props.compact }">
    <div class="openapi-payload-editor-toolbar">
      <span v-if="!compact" class="font-mono text-[10px] text-muted-foreground">{{ format === "text" ? "text/plain" : format }}</span>
      <Button v-if="!compact" size="xs" variant="outline" :disabled="format === 'text' || !modelValue.trim()" @click="formatBody">{{ t("specification.formatDocument") }}</Button>
      <OpenApiDynamicValuePicker side="bottom" @insert="insertDynamicValue" />
    </div>
    <div ref="host" class="openapi-payload-editor-host" />
    <textarea :value="modelValue" class="sr-only" :name="props.name" :aria-hidden="true" tabindex="-1" @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)" />
    <p v-if="error" class="openapi-payload-editor-error" role="alert">{{ error }}</p>
  </div>
</template>
