<script setup lang="ts">
import { computed, nextTick, ref, useId, useTemplateRef, watch, watchEffect } from "vue";
import { ArrowLeft, ChevronDown, ChevronRight, Plus, RefreshCw, Search, WandSparkles, X } from "@lucide/vue";
import { DialogRoot, DialogTitle, DialogTrigger, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { useMediaQuery } from "@vueuse/core";
import { useAppearancePortal } from "@/components/ui/useAppearancePortal";
import { Button } from "@/components/ui/button";
import AppSelect from "@/components/ui/AppSelect.vue";
import { DialogContent } from "@/components/ui/dialog";
import { useFloatingLayerOrder } from "@/components/ui/dialog/useDialogLayerOrder";
import { translate, type MessageKey } from "@/i18n";
import { buildDynamicGeneratorExpression, dynamicGeneratorCatalog, dynamicGeneratorFields, dynamicGeneratorProcessors, type GeneratorProcessor } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicGenerator";
import { createOpenApiVariableResolver } from "@/lib/specifications/openapi/workbench/variables/openapiDynamicVariables";
import OpenApiVariableField from "./OpenApiVariableField.vue";

const props = defineProps<{ disabled?: boolean; side?: "right" | "bottom" }>();
const emit = defineEmits<{ insert: [value: string] }>();
const t = translate;
const key = <K extends Extract<MessageKey, `specification.openapiDynamic.${string}`>>(name: K) => name;
const groups = {
  string: key("specification.openapiDynamic.groups.string"),
  lorem: key("specification.openapiDynamic.groups.lorem"),
  number: key("specification.openapiDynamic.groups.number"),
  date: key("specification.openapiDynamic.groups.date"),
  person: key("specification.openapiDynamic.groups.person"),
  phone: key("specification.openapiDynamic.groups.phone"),
  internet: key("specification.openapiDynamic.groups.internet"),
  image: key("specification.openapiDynamic.groups.image"),
  location: key("specification.openapiDynamic.groups.location"),
  helpers: key("specification.openapiDynamic.groups.helpers"),
  common: key("specification.openapiDynamic.groups.common"),
};
const methods: Record<string, MessageKey> = {
  "string.uuid": key("specification.openapiDynamic.methods.uuid"),
  "number.int": key("specification.openapiDynamic.methods.integer"),
  "number.float": key("specification.openapiDynamic.methods.float"),
  "date.timestamp": key("specification.openapiDynamic.methods.timestamp"),
  "person.fullName": key("specification.openapiDynamic.methods.fullName"),
  "person.firstName": key("specification.openapiDynamic.methods.firstName"),
  "person.lastName": key("specification.openapiDynamic.methods.lastName"),
  "internet.email": key("specification.openapiDynamic.methods.email"),
  "helpers.fromRegExp": key("specification.openapiDynamic.methods.pattern"),
};

const open = ref(false);
const search = ref("");
const expanded = ref(new Set<string>());
const method = ref("");
const values = ref<Record<string, string>>({});
const processors = ref<Array<GeneratorProcessor & { id: number }>>([]);
const custom = ref(false);
const customExpression = ref("");
const preview = ref("");
const error = ref(false);
let nextProcessorId = 0;
let restoreInput = false;
const portal = useAppearancePortal();
const { forwardRef } = useFloatingLayerOrder();
const compact = useMediaQuery("(max-width: 720px), (max-height: 560px), (max-width: 900px) and (pointer: coarse)");
const touch = useMediaQuery("(pointer: coarse)");
const visualBounds = ref<{ height: number; top: number }>();
const overlayProps = computed(() =>
  compact.value
    ? {
        showCloseButton: false,
        portalClass: "dynamic-generator-positioner",
        portalStyle: visualBounds.value && { blockSize: `${visualBounds.value.height}px`, insetBlockStart: `${visualBounds.value.top}px` },
      }
    : { side: props.side ?? "right", align: props.side === "bottom" ? "end" : "center", sideOffset: 8, collisionPadding: 10, hideWhenDetached: true },
);
const titleId = useId();
const expressionId = useId();
const searchInput = useTemplateRef("searchInput");
const methodButton = useTemplateRef("methodButton");
const methodList = useTemplateRef("methodList");
const processorControls = useTemplateRef("processorControls");
const heading = useTemplateRef("heading");

watchEffect((cleanup) => {
  if (!open.value || !compact.value || !window.visualViewport) return;
  const viewport = window.visualViewport;
  const update = () => {
    visualBounds.value = { height: viewport.height, top: viewport.offsetTop };
  };
  update();
  viewport.addEventListener("resize", update);
  viewport.addEventListener("scroll", update);
  cleanup(() => {
    viewport.removeEventListener("resize", update);
    viewport.removeEventListener("scroll", update);
    visualBounds.value = undefined;
  });
});

const fields = computed(() => dynamicGeneratorFields(method.value));
const query = computed(() => search.value.trim().toLowerCase());
const options = computed(() =>
  dynamicGeneratorCatalog.flatMap((category) => {
    if (!query.value || `${category.group} ${groupLabel(category.group)}`.toLowerCase().includes(query.value)) return [category];
    const matches = category.methods.filter(({ name }) => `${name.toLowerCase()} ${methodLabel(name).toLowerCase()}`.includes(query.value));
    return matches.length ? [{ ...category, methods: matches }] : [];
  }),
);
const draft = computed(() => {
  try {
    return { expression: custom.value ? customExpression.value.trim() : buildDynamicGeneratorExpression(method.value, values.value, processors.value), valid: true };
  } catch {
    return { expression: "", valid: false };
  }
});
const canInsert = computed(() => draft.value.valid && Boolean(draft.value.expression) && !error.value);
watch(draft, refresh);

function groupLabel(group: string) {
  const message = groups[group as keyof typeof groups];
  return message ? t(message) : group;
}

function methodLabel(name: string) {
  return methods[name] ? t(methods[name]!) : name.slice(name.indexOf(".") + 1);
}

function toggleGroup(group: string) {
  if (expanded.value.has(group)) expanded.value.delete(group);
  else expanded.value.add(group);
}

function refresh() {
  const { expression, valid } = draft.value;
  preview.value = "";
  error.value = !valid;
  if (!expression) return;
  try {
    if (!/^\{\{\s*\$[\s\S]+\}\}$/.test(expression)) throw new Error();
    preview.value = createOpenApiVariableResolver().expand(expression);
  } catch {
    error.value = true;
  }
}

function setOpen(value: boolean) {
  if (!value) {
    open.value = false;
    return;
  }
  search.value = "";
  expanded.value.clear();
  method.value = "";
  values.value = {};
  processors.value = [];
  custom.value = false;
  customExpression.value = "";
  restoreInput = false;
  open.value = true;
}

function focusPanel() {
  void nextTick(() => {
    if (!method.value && !custom.value && !(compact.value && touch.value)) searchInput.value?.focus({ preventScroll: true });
    else heading.value?.focus({ preventScroll: true });
  });
}

function restorePanelFocus(event: Event) {
  if (restoreInput || open.value) event.preventDefault();
}

function browse() {
  custom.value = false;
  method.value = "";
  search.value = "";
  void nextTick(() => searchInput.value?.focus());
}

function choose(name: string) {
  method.value = name;
  values.value = {};
  processors.value = [];
  void nextTick(() => methodButton.value?.focus());
}

function addProcessor() {
  processors.value.push({ id: ++nextProcessorId, name: "upperCase", value: "" });
  void nextTick(() =>
    processorControls.value
      ?.querySelectorAll<HTMLButtonElement>("button[data-processor]")
      .item(processors.value.length - 1)
      ?.focus(),
  );
}

function changeProcessor(row: GeneratorProcessor, value?: string) {
  row.name = value ?? "upperCase";
  row.value = processorFor(row.name)?.defaultValue ?? "";
}

function processorFor(name: string) {
  return dynamicGeneratorProcessors.find((processor) => processor.name === name);
}

function editExpression() {
  customExpression.value = draft.value.expression;
  custom.value = true;
  void nextTick(() => document.getElementById(expressionId)?.focus());
}

function insert() {
  if (!canInsert.value) return;
  restoreInput = true;
  emit("insert", draft.value.expression);
  open.value = false;
}
</script>

<template>
  <component :is="compact ? DialogRoot : PopoverRoot" :open="open" @update:open="setOpen">
    <component :is="compact ? DialogTrigger : PopoverTrigger" as-child>
      <button type="button" class="dynamic-value-trigger" :disabled="disabled" :aria-label="t('specification.openapiDynamic.title')" :title="t('specification.openapiDynamic.generator')">
        <WandSparkles :size="14" aria-hidden="true" />
      </button>
    </component>
    <component :is="compact ? 'div' : PopoverPortal" v-bind="compact ? { class: 'contents' } : { to: portal }">
      <component
        :is="compact ? DialogContent : PopoverContent"
        :ref="forwardRef"
        :class="['dynamic-value-picker', compact && 'dynamic-value-picker-compact']"
        :aria-labelledby="compact ? undefined : titleId"
        v-bind="overlayProps"
        @open-auto-focus.prevent="focusPanel"
        @close-auto-focus="restorePanelFocus"
      >
        <header ref="heading" class="dynamic-generator-heading" tabindex="-1">
          <WandSparkles :size="15" aria-hidden="true" />
          <DialogTitle v-if="compact" as="h3">{{ t("specification.openapiDynamic.generator") }}</DialogTitle>
          <h3 v-else :id="titleId">{{ t("specification.openapiDynamic.generator") }}</h3>
          <Button size="icon-xs" variant="ghost" :aria-label="t('specification.openapiDynamic.close')" @click="open = false"><X :size="14" aria-hidden="true" /></Button>
        </header>
        <div class="dynamic-generator-main">
          <span class="dynamic-generator-caption">{{ t("specification.openapiDynamic.type") }}</span>
          <template v-if="!method && !custom">
            <div class="dynamic-generator-search">
              <Search :size="14" aria-hidden="true" />
              <input
                ref="searchInput"
                v-model="search"
                class="field-control"
                :placeholder="t('specification.openapiDynamic.search')"
                :aria-label="t('specification.openapiDynamic.search')"
                name="dynamic-generator-search"
                autocomplete="off"
                @keydown.down.prevent="methodList?.querySelector('button')?.focus()"
              />
            </div>
            <div ref="methodList" class="dynamic-generator-catalog" :aria-label="t('specification.openapiDynamic.category')">
              <div v-for="category in options" :key="category.group" class="dynamic-generator-group">
                <button v-if="!query" type="button" class="dynamic-generator-group-trigger" :aria-expanded="expanded.has(category.group)" @click="toggleGroup(category.group)">
                  <component :is="expanded.has(category.group) ? ChevronDown : ChevronRight" :size="13" aria-hidden="true" />
                  <span>{{ groupLabel(category.group) }}</span
                  ><code v-if="category.group !== 'common'">${{ category.group }}</code>
                </button>
                <div v-if="query || expanded.has(category.group)" class="dynamic-generator-options">
                  <button v-for="item in category.methods" :key="item.name" type="button" @click="choose(item.name)">
                    <span>{{ methodLabel(item.name) }}</span
                    ><code>${{ item.name }}</code>
                  </button>
                </div>
              </div>
              <p v-if="!options.length" class="dynamic-generator-empty">{{ t("specification.openapiDynamic.noResults") }}</p>
            </div>
          </template>
          <template v-else>
            <button ref="methodButton" type="button" class="dynamic-generator-selected" @click="browse">
              <ArrowLeft :size="14" aria-hidden="true" />
              <code v-if="method && !custom">${{ method }}</code
              ><span v-else>{{ t("specification.openapiDynamic.editExpression") }}</span>
              <span class="dynamic-generator-change">{{ t("specification.openapiDynamic.changeType") }}</span>
            </button>
            <OpenApiVariableField v-if="custom" :id="expressionId" v-model="customExpression" class="dynamic-generator-expression-input" multiline name="dynamic-expression" :aria-label="t('specification.openapiDynamic.expression')" />
            <template v-else>
              <fieldset v-if="fields.length" class="dynamic-generator-arguments">
                <legend>{{ t("specification.openapiDynamic.parameters") }}</legend>
                <label v-for="field in fields" :key="field.name">
                  <span>{{ field.name }}</span>
                  <AppSelect v-if="field.kind === 'boolean' || field.choices" v-model="values[field.name]" size="form" :name="`dynamic-argument-${field.name}`">
                    <option value="">{{ t("specification.openapiDynamic.defaultValue") }}</option>
                    <option v-for="option in field.choices ?? ['true', 'false']" :key="option" :value="option">{{ option }}</option>
                  </AppSelect>
                  <input v-else v-model="values[field.name]" :name="`dynamic-argument-${field.name}`" class="field-control" :inputmode="field.kind === 'number' ? 'decimal' : undefined" :placeholder="t('specification.openapiDynamic.defaultValue')" autocomplete="off" spellcheck="false" />
                </label>
              </fieldset>
              <div ref="processorControls" class="dynamic-generator-processors">
                <div v-for="(row, index) in processors" :key="row.id" class="dynamic-generator-processor">
                  <AppSelect :model-value="row.name" size="compact" data-processor :aria-label="t('specification.openapiDynamic.processor')" @update:model-value="changeProcessor(row, $event)">
                    <option v-for="processor in dynamicGeneratorProcessors" :key="processor.name" :value="processor.name">{{ processor.name }}</option>
                  </AppSelect>
                  <input v-if="processorFor(row.name)?.input" v-model="row.value" class="field-control" :aria-label="t('specification.openapiDynamic.processorArgument')" :inputmode="processorFor(row.name)?.input === 'number' ? 'decimal' : undefined" autocomplete="off" />
                  <Button size="icon-xs" variant="ghost" :aria-label="t('specification.openapiDynamic.removeProcessor')" @click="processors.splice(index, 1)"><X :size="13" aria-hidden="true" /></Button>
                </div>
                <Button size="xs" variant="outline" @click="addProcessor"><Plus :size="13" aria-hidden="true" />{{ t("specification.openapiDynamic.addProcessor") }}</Button>
              </div>
            </template>
          </template>
        </div>
        <dl class="dynamic-generator-preview">
          <dt>{{ t("specification.openapiDynamic.expression") }}</dt>
          <dd>
            <code>{{ draft.expression }}</code>
          </dd>
          <dt>{{ t("specification.openapiDynamic.preview") }}</dt>
          <dd>
            <output>{{ preview }}</output
            ><Button size="icon-xs" variant="ghost" :aria-label="t('specification.openapiDynamic.refresh')" :disabled="!canInsert" @click="refresh"><RefreshCw :size="13" aria-hidden="true" /></Button>
          </dd>
        </dl>
        <p v-if="error" class="dynamic-generator-error" role="alert">{{ t("specification.openapiDynamic.invalid") }}</p>
        <footer class="dynamic-generator-footer">
          <Button v-if="!custom" variant="ghost" size="xs" @click="editExpression">{{ t("specification.openapiDynamic.editExpression") }}</Button>
          <Button variant="default" size="sm" class="dynamic-insert" :disabled="!canInsert" @click="insert">{{ t("specification.openapiDynamic.insert") }}</Button>
        </footer>
      </component>
    </component>
  </component>
</template>

<style scoped>
.dynamic-value-trigger {
  display: grid;
  place-items: center;
  inline-size: 24px;
  block-size: 24px;
  flex: 0 0 24px;
  padding: 0;
  border-radius: 5px;
  color: var(--muted-foreground);
}
.dynamic-value-trigger:hover,
.dynamic-value-trigger[aria-expanded="true"] {
  background: var(--accent);
  color: var(--foreground);
}
.dynamic-value-trigger:disabled {
  opacity: 0.45;
}
:global(.dynamic-value-picker) {
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: var(--specx-floating-layer-z-index, 51);
  inline-size: 336px;
  max-inline-size: min(calc(100vw - 20px), var(--reka-popover-content-available-width, 100vw));
  max-block-size: var(--reka-popover-content-available-height, 80vh);
  overflow: hidden;
  overscroll-behavior: contain;
  padding: 8px;
  color: var(--foreground);
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: 9px;
  box-shadow: 0 12px 38px rgb(0 0 0 / 18%);
  font-size: 12px;
}
:global(.dynamic-generator-positioner) {
  grid-template: minmax(0, 1fr) / minmax(0, 1fr);
  block-size: var(--specx-viewport-height);
  padding-block: max(8px, env(safe-area-inset-top)) max(8px, env(safe-area-inset-bottom));
  padding-inline: max(8px, env(safe-area-inset-left)) max(8px, env(safe-area-inset-right));
}
:global(.dynamic-value-picker.dynamic-value-picker-compact) {
  inline-size: min(100%, 32rem);
  max-inline-size: 100%;
  max-block-size: 100%;
  padding: 10px;
  container: generator / inline-size;
}
.dynamic-generator-heading {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 8px;
  padding-inline: 3px;
}
.dynamic-generator-heading h3 {
  flex: 1;
  min-inline-size: 0;
  font-size: 13px;
  font-weight: 650;
}
.dynamic-generator-main {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  padding: 8px;
  background: var(--surface-muted);
  border: 1px solid var(--border);
  border-radius: 6px;
}
.dynamic-generator-caption,
.dynamic-generator-arguments legend {
  font-weight: 600;
}
.dynamic-generator-search {
  position: relative;
  flex-shrink: 0;
}
.dynamic-generator-search > svg {
  position: absolute;
  inset-inline-start: 8px;
  inset-block-start: 8px;
  color: var(--muted-foreground);
  pointer-events: none;
}
.dynamic-generator-search > input {
  padding-inline-start: 28px;
}
.dynamic-generator-main :is(.field-control, .app-select) {
  width: 100%;
  min-width: 0;
  min-height: 30px;
  font-size: 12px;
}
.dynamic-generator-catalog {
  max-block-size: min(254px, 42vh);
  overflow: auto;
  overscroll-behavior: contain;
  padding: 3px;
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: 5px;
}
.dynamic-generator-catalog button,
.dynamic-generator-selected {
  display: flex;
  align-items: center;
  gap: 6px;
  inline-size: 100%;
  min-block-size: 28px;
  padding: 4px 6px;
  border-radius: 4px;
  text-align: start;
}
.dynamic-generator-catalog button:hover,
.dynamic-generator-selected:hover {
  background: var(--accent);
}
.dynamic-generator-catalog button span {
  min-inline-size: 0;
  flex: 1;
  overflow-wrap: anywhere;
}
.dynamic-generator-catalog code,
.dynamic-generator-change {
  color: var(--muted-foreground);
  font-size: 10px;
  overflow-wrap: anywhere;
}
.dynamic-generator-options button {
  padding-inline-start: 23px;
}
.dynamic-generator-selected {
  background: var(--popover);
  border: 1px solid var(--border);
}
.dynamic-generator-change {
  margin-inline-start: auto;
}
.dynamic-generator-empty {
  padding: 8px;
  color: var(--muted-foreground);
}
.dynamic-generator-arguments {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
  min-inline-size: 0;
}
.dynamic-generator-arguments legend {
  padding-block: 3px;
}
.dynamic-generator-arguments label {
  display: grid;
  gap: 2px;
  min-inline-size: 0;
  overflow-wrap: anywhere;
}
.dynamic-generator-processors {
  display: grid;
  gap: 4px;
  justify-items: start;
}
.dynamic-generator-processor {
  display: flex;
  align-items: center;
  gap: 4px;
  inline-size: 100%;
}
.dynamic-generator-processor :is(.app-select, input) {
  flex: 1;
}
.dynamic-generator-main :deep(.dynamic-generator-expression-input) {
  min-block-size: 62px;
  resize: vertical;
  font-family: var(--font-mono);
}
.dynamic-generator-preview {
  display: grid;
  flex-shrink: 0;
  grid-template-columns: max-content minmax(0, 1fr);
  align-items: baseline;
  gap: 4px 8px;
  margin: 0;
  padding: 6px 8px;
  border-inline-start: 2px solid var(--border);
  background: var(--surface-muted);
  border-radius: 4px;
}
.dynamic-generator-preview dt {
  color: var(--muted-foreground);
}
.dynamic-generator-preview dd {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 4px;
  min-inline-size: 0;
  min-block-size: 19px;
  margin: 0;
}
.dynamic-generator-preview :is(code, output) {
  min-inline-size: 0;
  max-block-size: 76px;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.dynamic-generator-error {
  margin: 0;
  color: var(--destructive);
}
.dynamic-generator-footer {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: end;
  flex-wrap: wrap;
  gap: 8px;
}
@container generator (max-width: 280px) {
  .dynamic-generator-arguments {
    grid-template-columns: minmax(0, 1fr);
  }
}
@media (max-height: 560px) {
  .dynamic-generator-preview :is(code, output) {
    max-block-size: 32px;
  }
}
@media (pointer: coarse) {
  .dynamic-generator-main :is(.field-control, .app-select),
  .dynamic-generator-main :deep(.dynamic-generator-expression-input) {
    font-size: 16px;
  }
  .dynamic-generator-catalog button,
  .dynamic-generator-selected,
  .dynamic-generator-main :is(.field-control, .app-select),
  .dynamic-generator-heading button,
  .dynamic-generator-footer button {
    min-block-size: 44px;
  }
}
</style>
