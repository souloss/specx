<script setup lang="ts">
import { computed } from "vue";
import { marked } from "marked";
import DOMPurify from "dompurify";

const props = defineProps<{ content: string }>();
const rendered = computed(() => DOMPurify.sanitize(marked.parse(props.content, { async: false })));
</script>

<template>
  <article class="markdown-preview" v-html="rendered" />
</template>
