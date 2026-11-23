<script setup lang="ts">
import { computed } from "vue";
import { highlightOpenApiCode } from "@/lib/specifications/openapi/snippets/openapiCodeHighlight";
const props = withDefaults(defineProps<{ text: string; language?: string }>(), { language: "text" });
const highlighted = computed(() => highlightOpenApiCode(props.text, props.language));
</script>
<template>
  <pre class="openapi-code-sample" tabindex="0" translate="no"><code v-html="highlighted" /></pre>
</template>

<style>
.openapi-code-sample {
  max-height: 30rem;
  overflow: auto;
  margin: 0;
  padding: 14px;
  background: var(--surface-muted);
  color: var(--foreground);
  font: 12px/1.75 var(--font-mono);
  tab-size: 2;
  white-space: pre;
}
:root,
[data-appearance="light"] {
  --openapi-code-string: #047857;
  --openapi-code-property: #1d4ed8;
  --openapi-code-number: #b45309;
  --openapi-code-keyword: #7e22ce;
}
.dark,
[data-appearance="dark"] {
  --openapi-code-string: #6ee7b7;
  --openapi-code-property: #93c5fd;
  --openapi-code-number: #fcd34d;
  --openapi-code-keyword: #d8b4fe;
}
.openapi-code-sample .hljs-comment {
  color: var(--muted-foreground);
  font-style: italic;
}
.openapi-code-sample .hljs-command {
  color: var(--openapi-code-property);
  font-weight: 650;
}
.openapi-code-sample .hljs-method {
  color: var(--openapi-code-keyword);
  font-weight: 700;
}
.openapi-code-sample .hljs-string,
.openapi-code-sample .hljs-string2 {
  color: var(--openapi-code-string);
}
.openapi-code-sample .hljs-attr,
.openapi-code-sample .hljs-title,
.openapi-code-sample .hljs-built_in {
  color: var(--openapi-code-property);
}
.openapi-code-sample .hljs-number,
.openapi-code-sample .hljs-literal,
.openapi-code-sample .hljs-type {
  color: var(--openapi-code-number);
}
.openapi-code-sample .hljs-keyword {
  color: var(--openapi-code-keyword);
}
</style>
