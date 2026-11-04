import { ref, watch } from "vue";
import { defineStore } from "pinia";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { isAssignableShortcut, normalizeShortcutBindings, SHORTCUT_ACTIONS, shortcutIdentity, type ShortcutActionId } from "@/lib/editor/shortcutRegistry";
import { matchesShortcut } from "@/lib/editor/keyboardShortcuts";
import { formatShortcutTooltip } from "@/lib/editor/shortcutDisplay";
import { translate } from "@/i18n";

export const SHORTCUT_STORAGE_KEY = "specx-shortcuts-v1";
export const useShortcutStore = defineStore("shortcuts", () => {
  let saved: unknown;
  try {
    saved = JSON.parse(safeLocalStorageGet(SHORTCUT_STORAGE_KEY) ?? "null");
  } catch {
    /* Ignore corrupt preferences. */
  }
  const bindings = ref(normalizeShortcutBindings(saved));
  watch(bindings, (value) => safeLocalStorageSet(SHORTCUT_STORAGE_KEY, JSON.stringify(value)), { deep: true });
  function set(id: ShortcutActionId, shortcut: string): string {
    if (!isAssignableShortcut(shortcut)) return translate("shortcuts.invalid");
    const conflict = SHORTCUT_ACTIONS.find((action) => action.id !== id && shortcut && bindings.value[action.id] && shortcutIdentity(bindings.value[action.id]) === shortcutIdentity(shortcut));
    if (conflict) return translate("shortcuts.conflict", { action: translate(conflict.label) });
    bindings.value[id] = shortcut;
    return "";
  }
  function reset(id: ShortcutActionId) {
    return set(id, SHORTCUT_ACTIONS.find((action) => action.id === id)!.defaultShortcut);
  }
  function resetAll() {
    bindings.value = normalizeShortcutBindings(null);
  }
  function matches(id: ShortcutActionId, event: KeyboardEvent) {
    return matchesShortcut(event, bindings.value[id]);
  }
  function hint(label: string, id: ShortcutActionId) {
    return formatShortcutTooltip(label, bindings.value[id]);
  }
  return { bindings, set, reset, resetAll, matches, hint };
});
