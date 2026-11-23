<script setup lang="ts">
import { computed } from "vue";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { translate } from "@/i18n";

const props = defineProps<{ content: string }>();
const t = translate;
const rendered = computed(() => {
  const html = marked.parse(props.content, { async: false, gfm: true, breaks: false });
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true } });
});
</script>

<template>
  <article class="markdown-preview" :aria-label="t('specification.preview')" v-html="rendered" />
</template>
