<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { ChevronRight, Pencil, RotateCcw, Search, X } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { SHORTCUT_ACTIONS, type ShortcutActionId } from "@/lib/editor/shortcutRegistry";
import { eventToShortcut } from "@/lib/editor/keyboardShortcuts";
import { formatShortcutDisplay, normalizeShortcutSearch } from "@/lib/editor/shortcutDisplay";
import { useShortcutStore } from "@/stores/shortcutStore";
import { translate as t } from "@/i18n";

const shortcuts = useShortcutStore();
const search = ref("");
const editing = ref<ShortcutActionId | null>(null);
const error = ref("");
const collapsed = ref(new Set<string>());
const groups = computed(() => {
  const needle = search.value.trim().toLocaleLowerCase();
  const shortcutNeedle = normalizeShortcutSearch(needle);
  return [
    { scope: "global", label: t("shortcuts.global") },
    { scope: "tree", label: t("shortcuts.tree") },
  ]
    .map((group) => ({
      ...group,
      actions: SHORTCUT_ACTIONS.filter((action) => {
        if (action.scope !== group.scope) return false;
        return !needle || `${t(action.label)} ${action.id} ${group.label}`.toLocaleLowerCase().includes(needle) || [shortcuts.bindings[action.id], formatShortcutDisplay(shortcuts.bindings[action.id])].some((value) => normalizeShortcutSearch(value).includes(shortcutNeedle));
      }),
    }))
    .filter((group) => group.actions.length);
});
function toggle(scope: string) {
  const next = new Set(collapsed.value);
  if (next.has(scope)) next.delete(scope);
  else next.add(scope);
  collapsed.value = next;
}
async function edit(id: ShortcutActionId) {
  error.value = "";
  editing.value = id;
  await nextTick();
  document.getElementById(`shortcut-${id}`)?.focus();
}
function record(event: KeyboardEvent, id: ShortcutActionId) {
  if (editing.value !== id || event.isComposing) return;
  if (event.key === "Tab" && !event.ctrlKey && !event.metaKey && !event.altKey) {
    editing.value = null;
    error.value = "";
    event.stopPropagation();
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  if (event.repeat) return;
  if (event.key === "Escape") {
    editing.value = null;
    error.value = "";
    return;
  }
  const value = event.key === "Backspace" && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey ? "" : eventToShortcut(event);
  if (value === null) {
    if (!["Control", "Meta", "Alt", "Shift"].includes(event.key)) error.value = t("shortcuts.invalid");
    return;
  }
  error.value = shortcuts.set(id, value);
  if (!error.value) editing.value = null;
}
function reset(id: ShortcutActionId) {
  editing.value = null;
  error.value = shortcuts.reset(id);
}
function disable(id: ShortcutActionId) {
  editing.value = null;
  error.value = shortcuts.set(id, "");
}
</script>

<template>
  <section class="space-y-4 py-2">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class="text-sm font-medium">{{ t("settings.shortcuts") }}</h2>
      <Button
        variant="outline"
        size="sm"
        @click="
          shortcuts.resetAll();
          editing = null;
          error = '';
        "
        ><RotateCcw class="mr-1 h-3 w-3" />{{ t("shortcuts.resetAll") }}</Button
      >
    </div>
    <p class="text-xs text-muted-foreground">{{ t("shortcuts.description") }}</p>
    <div class="relative">
      <Search class="pointer-events-none absolute left-2 top-2 h-4 w-4 text-muted-foreground" />
      <input v-model="search" class="h-8 w-full rounded border border-border bg-background pl-8 pr-8 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('shortcuts.search')" :placeholder="t('shortcuts.search')" />
      <button v-if="search" class="absolute right-2 top-2" :aria-label="t('settings.clearSearch')" @click="search = ''"><X class="h-4 w-4" /></button>
    </div>
    <p v-if="error" role="alert" class="text-xs text-destructive">{{ error }}</p>
    <p v-if="!groups.length" class="py-6 text-center text-xs text-muted-foreground">{{ t("settings.noMatches") }}</p>
    <section v-for="group in groups" :key="group.scope" class="rounded-md border border-border">
      <button class="flex w-full items-center gap-2 bg-muted/30 px-3 py-2 text-left text-xs font-medium" :aria-expanded="Boolean(search.trim()) || !collapsed.has(group.scope)" :aria-controls="`shortcut-group-${group.scope}`" @click="toggle(group.scope)">
        <ChevronRight class="h-3 w-3" :class="{ 'rotate-90': search.trim() || !collapsed.has(group.scope) }" />{{ group.label }}<span class="ml-auto text-muted-foreground">{{ group.actions.length }}</span>
      </button>
      <div v-show="search.trim() || !collapsed.has(group.scope)" :id="`shortcut-group-${group.scope}`">
        <div v-for="action in group.actions" :key="action.id" class="flex flex-wrap items-center gap-2 border-t border-border px-3 py-2">
          <label :for="`shortcut-${action.id}`" class="min-w-32 flex-1 text-xs"
            >{{ t(action.label) }}<small v-if="shortcuts.bindings[action.id] !== action.defaultShortcut" class="ml-2 text-muted-foreground">{{ t("shortcuts.modified") }}</small></label
          >
          <input
            :id="`shortcut-${action.id}`"
            data-shortcut-recorder
            readonly
            :value="editing === action.id ? t('shortcuts.recording') : shortcuts.bindings[action.id] ? formatShortcutDisplay(shortcuts.bindings[action.id]) : t('shortcuts.disabled')"
            :aria-label="t('shortcuts.binding', { action: t(action.label) })"
            :aria-describedby="editing === action.id ? 'shortcut-record-help' : undefined"
            class="h-7 w-40 cursor-pointer rounded border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            @focus="
              editing = action.id;
              error = '';
            "
            @click="edit(action.id)"
            @keydown="record($event, action.id)"
            @blur="editing === action.id && (editing = null)"
          />
          <button class="rounded p-1 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('shortcuts.edit', { action: t(action.label) })" @click="edit(action.id)"><Pencil class="h-3.5 w-3.5" /></button>
          <button class="rounded p-1 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('shortcuts.reset', { action: t(action.label) })" :disabled="shortcuts.bindings[action.id] === action.defaultShortcut" @click="reset(action.id)"><RotateCcw class="h-3.5 w-3.5" /></button>
          <button class="rounded p-1 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" :aria-label="t('shortcuts.disable', { action: t(action.label) })" :disabled="!shortcuts.bindings[action.id]" @click="disable(action.id)"><X class="h-3.5 w-3.5" /></button>
        </div>
      </div>
    </section>
    <p id="shortcut-record-help" class="text-xs text-muted-foreground">{{ t("shortcuts.recordHelp") }}</p>
    <p class="text-xs text-muted-foreground">{{ t("shortcuts.nativeNavigation") }}</p>
  </section>
</template>
