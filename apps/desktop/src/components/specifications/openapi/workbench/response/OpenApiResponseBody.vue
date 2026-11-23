<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Brackets, ChevronDown, ChevronUp, ChevronsDownUp, ChevronsUpDown, Clipboard, Filter, Search, WrapText } from "@lucide/vue";
import { xml as xmlLanguage } from "@codemirror/lang-xml";
import { html as htmlLanguage } from "@codemirror/lang-html";
import { json as jsonLanguage } from "@codemirror/lang-json";
import { HighlightStyle, foldAll, foldedRanges, foldGutter, foldKeymap, syntaxHighlighting, syntaxTree, unfoldAll, unfoldEffect } from "@codemirror/language";
import { Compartment, EditorState, RangeSetBuilder, StateEffect, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, keymap, lineNumbers, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { drawSelection, highlightActiveLine, highlightActiveLineGutter } from "@codemirror/view";
import { ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem } from "reka-ui";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";
import { tags } from "@lezer/highlight";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import { translate as t } from "@/i18n";
import { findResponseMatches, firstVisibleResponseMatch, responseMatchAt, selectedResponseJson } from "@/lib/specifications/openapi/workbench/response/openapiResponse";
import { filterOpenApiResponse, type ResponseFilterResult } from "@/lib/specifications/openapi/workbench/response/openapiResponseFilter";

const props = defineProps<{ text: string; json: boolean; canFormat: boolean; original: boolean; filterSource?: string; compact?: boolean; minimal?: boolean; syntax?: string }>();
const emit = defineEmits<{ "update:original": [value: boolean] }>();
// Null exports the normal body view. An empty string represents a filter without output.
const filteredText = defineModel<string | null>("filteredText");
const filterOpen = ref(false);
const filterQuery = ref("");
const filterInput = ref<HTMLInputElement>();
const filterResult = ref<ResponseFilterResult>();
const filtering = ref(false);
let filterAbort: AbortController | undefined;
const filterHasOutput = computed(() => Boolean(filterResult.value && "count" in filterResult.value && filterResult.value.count));
const filterError = computed(() => (filterResult.value && "error" in filterResult.value ? filterResult.value : null));
const displayText = computed(() => (filterOpen.value ? (filteredText.value ?? "") : props.text));
const host = ref<HTMLElement>();
const searchText = ref("");
const searchOpen = ref(false);
const matchCount = ref(0);
const activeMatch = ref(0);
const wrap = ref(true);
const portal = useAppearancePortal();
const { forwardRef } = useFloatingLayerOrder();
const contextSelection = ref("");
const contextLine = ref("");
const path = ref("");
const clipboardStatus = ref("");
const wrapping = new Compartment();
let view: EditorView | undefined;
let offsets: Uint32Array = new Uint32Array();
let searchDocument = "";

function clearFilter() {
  filterAbort?.abort();
  filterAbort = undefined;
  filtering.value = false;
  filterResult.value = undefined;
  filteredText.value = filterOpen.value ? "" : null;
}

function showFilter() {
  if (!filterOpen.value) {
    filterOpen.value = true;
    clearFilter();
  }
  void nextTick(() => filterInput.value?.focus());
}

function cancelFilter() {
  clearFilter();
  filterInput.value?.focus();
}

function showBody(original: boolean) {
  filterOpen.value = false;
  clearFilter();
  emit("update:original", original);
}

async function applyFilter() {
  const query = filterQuery.value.trim();
  if (!props.filterSource || !query) return;
  clearFilter();
  const controller = new AbortController();
  filterAbort = controller;
  filtering.value = true;
  const result = await filterOpenApiResponse(props.filterSource, query, controller.signal);
  if (controller.signal.aborted) return;
  filterAbort = undefined;
  filtering.value = false;
  filterResult.value = result;
  filteredText.value = "text" in result ? result.text : "";
}

type SearchState = { offsets: Uint32Array; active: number };
const setMatches = StateEffect.define<SearchState>();
const searchState = StateField.define<SearchState>({
  create: () => ({ offsets: new Uint32Array(), active: -1 }),
  update: (value, transaction) => transaction.effects.find((effect) => effect.is(setMatches))?.value ?? value,
});
const matchMark = Decoration.mark({ class: "response-search-match" });
const activeMark = Decoration.mark({ class: "response-search-match active" });
const searchHighlight = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(editor: EditorView) {
      this.decorations = this.build(editor);
    }
    update(update: ViewUpdate) {
      if (update.viewportChanged || update.docChanged || update.startState.field(searchState) !== update.state.field(searchState)) this.decorations = this.build(update.view);
    }
    build(editor: EditorView): DecorationSet {
      const { offsets, active } = editor.state.field(searchState);
      const builder = new RangeSetBuilder<Decoration>();
      for (const { from, to } of editor.visibleRanges) {
        let index = firstVisibleResponseMatch(offsets, from);
        const limit = index + 2000;
        for (; index < offsets.length / 2 && offsets[index * 2]! < to; index += 1) {
          if (index >= limit && index !== active) {
            if (active < index || offsets[active * 2]! >= to) break;
            index = active;
          }
          builder.add(Math.max(from, offsets[index * 2]!), Math.min(to, offsets[index * 2 + 1]!), index === active ? activeMark : matchMark);
        }
      }
      return builder.finish();
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

const matchLabel = computed(() => (searchText.value.trim() ? t("specification.openapiSearchMatchCount", { current: matchCount.value ? activeMatch.value + 1 : 0, total: matchCount.value }) : ""));
const responseJsonHighlight = HighlightStyle.define([
  { tag: tags.propertyName, class: "response-code-key" },
  { tag: tags.string, class: "response-code-string" },
  { tag: tags.number, class: "response-code-literal" },
  { tag: [tags.bool, tags.null], class: "response-code-keyword" },
]);
const editorTheme = EditorView.theme({
  "&": { color: "var(--foreground)", backgroundColor: "var(--background)" },
  ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono)", lineHeight: "16px" },
  ".cm-gutters": { backgroundColor: "var(--background)", borderRight: "none", color: "var(--muted-foreground)" },
  ".cm-content": { padding: "8px 0", minHeight: "7rem", caretColor: "var(--foreground)" },
  ".cm-line": { padding: "0 8px" },
  ".cm-foldPlaceholder": { color: "var(--muted-foreground)", background: "var(--surface-muted)", borderColor: "var(--border)" },
  ".cm-focused": { outlineColor: "var(--primary)" },
  ".cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 24%, transparent) !important" },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "color-mix(in srgb, var(--accent) 45%, transparent)" },
});

function updateSelection(state: EditorState) {
  path.value = props.json ? (selectedResponseJson(state)?.path ?? "") : "";
}

function selectLine() {
  if (!view) return;
  const line = view.state.doc.lineAt(view.state.selection.main.head);
  view.dispatch({ selection: { anchor: line.from, head: line.to } });
  view.focus();
}
function selectAll() {
  view?.dispatch({ selection: { anchor: 0, head: view.state.doc.length } });
  view?.focus();
}
function prepareContextMenu(event: MouseEvent) {
  if (!view) return;
  const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
  const selection = view.state.selection.main;
  if (pos !== null && (pos < selection.from || pos > selection.to || selection.empty)) view.dispatch({ selection: { anchor: pos } });
  const current = view.state.selection.main;
  contextSelection.value = view.state.sliceDoc(current.from, current.to);
  contextLine.value = view.state.doc.lineAt(current.head).text;
}
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    clipboardStatus.value = t("specification.openapiResponseValueCopied");
  } catch {
    clipboardStatus.value = t("specification.openapiCopyFailed");
  }
}

function revealMatch(position: number, scroll = true) {
  if (!view) return;
  const match = responseMatchAt(offsets, position);
  if (!match) {
    view.dispatch({ effects: setMatches.of({ offsets, active: -1 }) });
    return;
  }
  activeMatch.value = position;
  const unfolds: StateEffect<unknown>[] = [];
  foldedRanges(view.state).between(match.from, match.to, (from, to) => {
    unfolds.push(unfoldEffect.of({ from, to }));
  });
  view.dispatch({
    effects: [...unfolds, setMatches.of({ offsets, active: position }), ...(scroll ? [EditorView.scrollIntoView(match.from, { y: "center" })] : [])],
    selection: { anchor: match.from, head: match.to },
  });
}

function rebuildMatches(scroll: boolean) {
  if (!view) return;
  offsets = findResponseMatches(searchDocument, searchText.value.trim());
  matchCount.value = offsets.length / 2;
  activeMatch.value = 0;
  revealMatch(0, scroll);
}

function navigateMatch(direction: -1 | 1) {
  if (matchCount.value) revealMatch((activeMatch.value + direction + matchCount.value) % matchCount.value);
}

function handleSearchKey(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === "Enter") {
    event.preventDefault();
    navigateMatch(event.shiftKey ? -1 : 1);
  } else if (event.key === "Escape") {
    event.preventDefault();
    searchText.value = "";
    view?.focus();
  }
}

function foldResponse(all: boolean) {
  if (!view) return;
  if (all) foldAll(view);
  else unfoldAll(view);
}

async function copyField(kind: "path" | "value") {
  if (!view) return;
  const selected = selectedResponseJson(view.state);
  if (!selected) return;
  try {
    await navigator.clipboard.writeText(kind === "path" ? selected.path : view.state.sliceDoc(selected.from, selected.to));
    clipboardStatus.value = t(kind === "path" ? "specification.openapiResponsePathCopied" : "specification.openapiResponseValueCopied");
  } catch {
    clipboardStatus.value = t("specification.openapiCopyFailed");
  }
}

function createEditor() {
  if (!host.value) return;
  const editorWasFocused = view?.hasFocus;
  view?.destroy();
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: displayText.value,
      extensions: [
        EditorState.readOnly.of(true),
        EditorView.contentAttributes.of({ "aria-label": t("specification.openapiResponseBody"), "aria-readonly": "true", spellcheck: "false" }),
        drawSelection(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        lineNumbers({
          domEventHandlers: {
            mousedown(editor, line, event) {
              if (!(event instanceof MouseEvent) || event.button !== 0) return false;
              const source = editor.state.doc.lineAt(line.from);
              editor.dispatch({ selection: { anchor: source.from, head: source.to } });
              editor.focus();
              return true;
            },
          },
        }),
        ...(props.json ? [jsonLanguage(), syntaxHighlighting(responseJsonHighlight), foldGutter({ openText: "⌄", closedText: "›" }), keymap.of(foldKeymap)] : []),
        ...(!props.json && props.syntax === "xml" ? [xmlLanguage()] : !props.json && props.syntax === "html" ? [htmlLanguage()] : []),
        wrapping.of(wrap.value ? EditorView.lineWrapping : []),
        searchState,
        searchHighlight,
        editorTheme,
        EditorView.updateListener.of((update) => {
          if (update.selectionSet || syntaxTree(update.startState) !== syntaxTree(update.state)) {
            if (update.selectionSet) clipboardStatus.value = "";
            updateSelection(update.state);
          }
        }),
      ],
    }),
  });
  searchDocument = view.state.doc.toString();
  updateSelection(view.state);
  clipboardStatus.value = "";
  rebuildMatches(Boolean(searchText.value.trim()));
  if (editorWasFocused) view.focus();
}

onMounted(createEditor);
watch([displayText, () => props.json, () => props.syntax], createEditor);
watch(
  () => props.filterSource,
  () => {
    filterOpen.value = false;
    filterQuery.value = "";
    clearFilter();
  },
);
watch(filterQuery, () => {
  if (filterOpen.value) clearFilter();
});
watch(searchText, () => rebuildMatches(true));
watch(wrap, (value) => view?.dispatch({ effects: wrapping.reconfigure(value ? EditorView.lineWrapping : []) }));
onBeforeUnmount(() => {
  filterAbort?.abort();
  view?.destroy();
  filteredText.value = null;
});
</script>

<template>
  <div class="openapi-response-viewer">
    <div v-if="!minimal" class="response-viewer-toolbar" :class="{ compact }">
      <slot name="leading" />
      <div v-if="!compact && (canFormat || filterSource || $slots.modes)" class="response-format-switch" role="group" :aria-label="t('specification.openapiResponseFormat')">
        <button v-if="canFormat" type="button" :aria-pressed="!filterOpen && !original" @click="showBody(false)"><Brackets class="h-3 w-3" aria-hidden="true" />{{ t("specification.openapiResponseFormatted") }}</button>
        <button type="button" :aria-pressed="!filterOpen && (original || !canFormat)" @click="showBody(true)">{{ t("specification.openapiResponseOriginal") }}</button>
        <button v-if="filterSource" type="button" :aria-pressed="filterOpen" @click="showFilter"><Filter class="h-3 w-3" aria-hidden="true" />{{ t("specification.openapiResponseFilter.mode") }}</button>
        <slot name="modes" />
      </div>
      <div class="response-viewer-actions">
        <template v-if="compact">
          <button v-if="filterSource" type="button" class="icon-button" :aria-label="t('specification.openapiResponseFilter.mode')" :aria-pressed="filterOpen" @click="filterOpen ? showBody(false) : showFilter()"><Filter class="size-3.5" /></button
          ><button type="button" class="icon-button" :aria-label="t('specification.openapiSearchResponse')" @click="searchOpen = !searchOpen"><Search class="size-3.5" /></button>
        </template>
        <AppTooltip v-if="json && (filterOpen ? filterHasOutput : !original)" :text="t('specification.openapiResponseFoldAll')">
          <button type="button" class="icon-button" :aria-label="t('specification.openapiResponseFoldAll')" @click="foldResponse(true)"><ChevronsDownUp class="h-3.5 w-3.5" aria-hidden="true" /></button>
        </AppTooltip>
        <AppTooltip v-if="json && (filterOpen ? filterHasOutput : !original)" :text="t('specification.openapiResponseUnfoldAll')">
          <button type="button" class="icon-button" :aria-label="t('specification.openapiResponseUnfoldAll')" @click="foldResponse(false)"><ChevronsUpDown class="h-3.5 w-3.5" aria-hidden="true" /></button>
        </AppTooltip>
        <AppTooltip v-if="!filterOpen || filterHasOutput" :text="t('specification.openapiResponseWrap')">
          <button type="button" class="icon-button" :aria-label="t('specification.openapiResponseWrap')" :aria-pressed="wrap" @click="wrap = !wrap"><WrapText class="h-3.5 w-3.5" aria-hidden="true" /></button>
        </AppTooltip>
        <slot name="actions" />
      </div>
    </div>
    <form v-if="filterOpen" class="response-filter" @submit.prevent="applyFilter">
      <label class="response-filter-label" for="openapi-response-filter">{{ t("specification.openapiResponseFilter.label") }}</label>
      <div class="response-filter-query">
        <input
          id="openapi-response-filter"
          ref="filterInput"
          v-model="filterQuery"
          class="field-control field-control-compact font-mono"
          name="openapi-response-filter"
          autocomplete="off"
          spellcheck="false"
          maxlength="2000"
          placeholder=".items[] | .name"
          :aria-invalid="filterError?.error === 'syntax'"
          aria-describedby="openapi-response-filter-help"
        />
        <button type="submit" class="debug-add-button" :disabled="!filterQuery.trim() || filtering">{{ t("specification.openapiResponseFilter.run") }}</button>
        <button v-if="filtering" type="button" class="debug-add-button" @click="cancelFilter">{{ t("common.cancel") }}</button>
      </div>
      <p id="openapi-response-filter-help">{{ t("specification.openapiResponseFilter.help") }}</p>
      <p v-if="filtering" role="status">{{ t("specification.openapiResponseFilter.running") }}</p>
      <div v-else-if="filterError" class="response-filter-error" role="alert">
        <p>{{ t(`specification.openapiResponseFilter.${filterError.error}`) }}</p>
        <pre v-if="filterError.detail" translate="no">{{ filterError.detail }}</pre>
      </div>
      <p v-else-if="filterResult && 'count' in filterResult" role="status">{{ filterResult.count ? t("specification.openapiResponseFilter.results", { count: filterResult.count }) : t("specification.openapiResponseFilter.empty") }}</p>
    </form>
    <div v-show="(!compact || searchOpen) && (!filterOpen || filterHasOutput)" class="response-search">
      <label class="response-search-control">
        <Search class="h-3.5 w-3.5" aria-hidden="true" />
        <span class="sr-only">{{ t("specification.openapiSearchResponse") }}</span>
        <input id="openapi-response-search" v-model="searchText" name="openapi-response-search" class="field-control field-control-compact" type="search" :placeholder="t('specification.openapiSearchResponse')" autocomplete="off" spellcheck="false" @keydown="handleSearchKey" />
      </label>
      <span v-if="matchLabel" class="response-search-count" aria-live="polite">{{ matchLabel }}</span>
      <AppTooltip :text="t('specification.openapiPreviousMatch')">
        <button type="button" class="icon-button" :aria-label="t('specification.openapiPreviousMatch')" :disabled="!matchCount" @click="navigateMatch(-1)"><ChevronUp class="h-3.5 w-3.5" aria-hidden="true" /></button>
      </AppTooltip>
      <AppTooltip :text="t('specification.openapiNextMatch')">
        <button type="button" class="icon-button" :aria-label="t('specification.openapiNextMatch')" :disabled="!matchCount" @click="navigateMatch(1)"><ChevronDown class="h-3.5 w-3.5" aria-hidden="true" /></button>
      </AppTooltip>
    </div>
    <ContextMenuRoot>
      <ContextMenuTrigger as-child><div v-show="!filterOpen || filterHasOutput" ref="host" class="response-code-viewer" @contextmenu.capture="prepareContextMenu" /></ContextMenuTrigger>
      <ContextMenuPortal :to="portal"
        ><ContextMenuContent :ref="forwardRef" class="response-editor-menu openapi-workbench-surface cn-menu-translucent z-(--specx-floating-layer-z-index)" @close-auto-focus.prevent="view?.focus()">
          <ContextMenuItem :disabled="!contextSelection" @select="copyText(contextSelection)">{{ t("specification.openapiResponseCopySelection") }}</ContextMenuItem>
          <ContextMenuItem @select="copyText(contextLine)">{{ t("specification.openapiResponseCopyLine") }}</ContextMenuItem>
          <ContextMenuItem v-if="json" :disabled="!path" @select="copyField('path')">{{ t("specification.openapiResponseCopyPath") }}</ContextMenuItem>
          <ContextMenuItem v-if="json" :disabled="!path" @select="copyField('value')">{{ t("specification.openapiResponseCopyValue") }}</ContextMenuItem>
          <ContextMenuItem @select="selectLine">{{ t("specification.openapiResponseSelectLine") }}</ContextMenuItem>
          <ContextMenuItem @select="selectAll">{{ t("specification.openapiResponseSelectAll") }}</ContextMenuItem>
        </ContextMenuContent></ContextMenuPortal
      >
    </ContextMenuRoot>
    <div v-if="!compact && json && (!filterOpen || filterHasOutput)" class="response-field-actions">
      <code v-if="path" :title="path" translate="no">{{ path }}</code>
      <span v-else>{{ t("specification.openapiResponseSelectField") }}</span>
      <div class="response-viewer-actions">
        <button type="button" class="debug-add-button" :disabled="!path" @click="copyField('path')">{{ t("specification.openapiResponseCopyPath") }}</button>
        <button type="button" class="debug-add-button" :disabled="!path" @click="copyField('value')"><Clipboard class="h-3 w-3" aria-hidden="true" />{{ t("specification.openapiResponseCopyValue") }}</button>
      </div>
    </div>
    <p v-if="clipboardStatus" class="response-field-status" role="status">{{ clipboardStatus }}</p>
  </div>
</template>
