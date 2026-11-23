<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup, EditorView } from "codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { EditorState } from "@codemirror/state";
import { openApiScriptDynamicCompletion } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicCompletion";

const props = defineProps<{ modelValue: string; label: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const host = ref<HTMLElement>();
let editor: EditorView | undefined;

onMounted(() => {
  if (!host.value) return;
  editor = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        javascript(),
        openApiScriptDynamicCompletion,
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ "aria-label": props.label, spellcheck: "false" }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) emit("update:modelValue", update.state.doc.toString());
        }),
        EditorView.theme({
          "&": { minHeight: "12rem", color: "var(--foreground)", backgroundColor: "transparent" },
          ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono, ui-monospace, monospace)" },
          ".cm-gutters": { backgroundColor: "var(--background)", borderRight: "1px solid var(--border)", color: "var(--muted-foreground)" },
          ".cm-activeLineGutter, .cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 55%, transparent)" },
          ".cm-focused": { outline: "none" },
          ".cm-content": { minHeight: "12rem", padding: "0.7rem 0" },
          ".cm-line": { paddingLeft: "0.7rem" },
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

onBeforeUnmount(() => editor?.destroy());
</script>

<template>
  <div ref="host" class="openapi-script-editor" />
</template>
