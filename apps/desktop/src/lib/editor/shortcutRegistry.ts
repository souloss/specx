import type { MessageKey } from "@/i18n";
import { isMacShortcutPlatform, parseShortcutParts } from "./shortcutDisplay";

type ShortcutDefinition = { id: string; label: MessageKey; scope: "global" | "tree"; defaultShortcut: string };
export const SHORTCUT_ACTIONS = [
  { id: "settings", label: "settings.shortcutSettings", scope: "global", defaultShortcut: "Mod+," },
  { id: "sidebar", label: "settings.shortcutSidebar", scope: "global", defaultShortcut: "Mod+B" },
  { id: "search", label: "settings.shortcutSearch", scope: "global", defaultShortcut: "Mod+K" },
  { id: "locate", label: "settings.shortcutLocate", scope: "global", defaultShortcut: "Shift+Mod+L" },
  { id: "newService", label: "settings.shortcutNewService", scope: "global", defaultShortcut: "Mod+N" },
  { id: "closeTab", label: "settings.shortcutCloseTab", scope: "global", defaultShortcut: "Mod+W" },
  { id: "rename", label: "settings.shortcutRename", scope: "tree", defaultShortcut: "F2" },
  { id: "delete", label: "settings.shortcutDelete", scope: "tree", defaultShortcut: "Delete" },
] as const satisfies readonly ShortcutDefinition[];
export type ShortcutActionId = (typeof SHORTCUT_ACTIONS)[number]["id"];
export type ShortcutBindings = Record<ShortcutActionId, string>;

/** Compare actual platform modifiers, independent of storage/display order. */
export function shortcutIdentity(shortcut: string, platform = globalThis.navigator?.platform || "") {
  return parseShortcutParts(shortcut)
    .map((part) => (part === "Mod" ? (isMacShortcutPlatform(platform) ? "Meta" : "Ctrl") : part))
    .map((part) => part.toLowerCase())
    .sort()
    .join("+");
}

export function isAssignableShortcut(shortcut: string): boolean {
  if (!shortcut) return true;
  const parts = parseShortcutParts(shortcut);
  const key = parts.at(-1) ?? "";
  if (!key || !/^(?:[A-Z0-9]|F(?:[1-9]|1[0-2])|Delete|Backspace|Space|Plus|[[\]\\;,./'`=-])$/i.test(key)) return false;
  const modifiers = parts.slice(0, -1);
  if (new Set(modifiers).size !== modifiers.length || modifiers.some((part) => !["Mod", "Ctrl", "Meta", "Alt", "Shift"].includes(part))) return false;
  // Plain text, native navigation and common browser/OS commands stay available.
  if (!modifiers.length) return (/^F(?:[1-9]|1[0-2])$/.test(key) && !["F5", "F11", "F12"].includes(key)) || key === "Delete";
  if (modifiers.every((part) => part === "Shift")) return false;
  const identity = shortcutIdentity(shortcut);
  return !["Mod+R", "Shift+Mod+R", "Mod+L", "Mod+T", "Alt+F4", "Mod+Q", "Mod+Tab", "Mod+Space", "Alt+Space", "Mod+A", "Mod+C", "Mod+V", "Mod+X", "Mod+Z", "Shift+Mod+Z", "Mod+Y"].some((reserved) => shortcutIdentity(reserved) === identity);
}

export function normalizeShortcutBindings(input: unknown): ShortcutBindings {
  const stored = input && typeof input === "object" ? (input as Record<string, unknown>) : {};
  const defaults = Object.fromEntries(SHORTCUT_ACTIONS.map((action) => [action.id, action.defaultShortcut])) as ShortcutBindings;
  const bindings = { ...defaults };
  for (const action of SHORTCUT_ACTIONS) {
    const value = stored[action.id];
    if (typeof value === "string" && isAssignableShortcut(value)) bindings[action.id] = value;
  }
  const identities = Object.values(bindings)
    .filter(Boolean)
    .map((shortcut) => shortcutIdentity(shortcut));
  // Invalid external edits must never make two actions fire for one keystroke.
  return new Set(identities).size === identities.length ? bindings : defaults;
}
