/* Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. */
import { isMacShortcutPlatform, parseShortcutParts } from "@/lib/editor/shortcutDisplay";

export interface ShortcutLikeEvent {
  key: string;
  code?: string;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  isComposing?: boolean;
}

function normalizeKey(key: string): string {
  if (key === " ") return "Space";
  if (key === "+" || key === "Plus") return "Plus";
  return key.length === 1 ? key.toLowerCase() : key;
}

// Physical keys whose unshifted character CodeMirror always registers bindings
// under. `KeyboardEvent.key` reports the *shifted* character instead ("/" -> "?",
// "1" -> "!"), so a shortcut recorded from that character can never match the
// binding CodeMirror looks up. Letters are excluded: they already round-trip
// because `shortcutToCodeMirrorKey` lowercases them.
const BASE_KEY_BY_CODE: Record<string, string> = {
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

/**
 * The unshifted character for a shifted punctuation/digit press, or null when
 * the event is not one. Mirrors CodeMirror's `base[event.keyCode]` table, which
 * is what its keymap resolves `Shift-` bindings against.
 */
function baseKeyForShiftedEvent(event: ShortcutLikeEvent): string | null {
  if (!event.shiftKey || event.key.length !== 1) return null;
  const baseKey = BASE_KEY_BY_CODE[event.code ?? ""];
  return baseKey && baseKey !== event.key ? baseKey : null;
}

function matchesShortcutKey(event: ShortcutLikeEvent, key: string, platform = globalThis.navigator?.platform || ""): boolean {
  if (normalizeKey(event.key) === normalizeKey(key)) return true;
  // Shortcuts store the unshifted character for shifted punctuation/digits (see
  // baseKeyForShiftedEvent), so match those against the physical key too.
  if (baseKeyForShiftedEvent(event) === normalizeKey(key)) return true;
  // KeyboardEvent.code 回退：event.key 随布局/修饰键变形时按物理键位匹配
  // （macOS Option+字母 → 变形字符如 ⌥W="∑"；俄文等非拉丁布局 → "Ц"）。
  // 仅限 Alt 组合键场景
  if (!event.altKey || !/^Key[A-Z]$/.test(event.code ?? "") || !/^[A-Z]$/i.test(key)) return false;
  // 非 macOS 的 Ctrl+Alt 是 AltGr 特征：用户可能在输入字符（如波兰语
  // AltGr+W → "ł"），按 code 强制匹配会让全局快捷键在打字时误触发
  if (!isMacShortcutPlatform(platform) && event.ctrlKey) return false;
  return event.code!.slice(3).toLowerCase() === key.toLowerCase();
}

function shortcutKeyName(key: string): string | null {
  if (key === " ") return "Space";
  if (key === "+") return "Plus";
  if (["Control", "Meta", "Shift", "Alt"].includes(key)) return null;
  if (key.length === 1) return key.toUpperCase();
  return key;
}

function shortcutKeyNameFromEvent(event: ShortcutLikeEvent, platform: string): string | null {
  if (event.altKey && isMacShortcutPlatform(platform) && /^Key[A-Z]$/.test(event.code ?? "")) {
    return event.code!.slice(3);
  }
  // Record "Shift+Mod+/" rather than "Shift+Mod+?" so the CodeMirror binding
  // ("Shift-Ctrl-/") is one the keymap actually resolves for this key press.
  const baseKey = baseKeyForShiftedEvent(event);
  if (baseKey) return shortcutKeyName(baseKey);
  return shortcutKeyName(event.key);
}

export function eventToShortcut(event: ShortcutLikeEvent, platform = globalThis.navigator?.platform || ""): string | null {
  if (event.isComposing) return null;

  const key = shortcutKeyNameFromEvent(event, platform);
  if (!key) return null;

  const hasModifier = !!event.metaKey || !!event.ctrlKey || !!event.altKey || !!event.shiftKey;
  if (!hasModifier && event.key.length === 1 && event.key !== " ") return null;

  const isMac = isMacShortcutPlatform(platform);
  const parts: string[] = [];
  if (event.shiftKey) parts.push("Shift");
  if (isMac) {
    if (event.ctrlKey) parts.push("Ctrl");
    if (event.metaKey) parts.push("Mod");
  } else if (event.ctrlKey && event.metaKey) {
    parts.push("Mod", "Meta");
  } else if (event.ctrlKey || event.metaKey) {
    parts.push("Mod");
  }
  if (event.altKey) parts.push("Alt");
  parts.push(key);
  return parts.join("+");
}

export function matchesShortcut(event: ShortcutLikeEvent, shortcut: string, platform = globalThis.navigator?.platform || ""): boolean {
  if (event.isComposing || !shortcut) return false;
  const parts = parseShortcutParts(shortcut);
  const key = parts[parts.length - 1] ?? "";
  const modifiers = new Set(parts.slice(0, -1));
  const usesMod = modifiers.has("Mod");
  const usesMeta = modifiers.has("Meta");
  const usesCtrl = modifiers.has("Ctrl");

  if (usesMod) {
    if (isMacShortcutPlatform(platform)) {
      if (!event.metaKey || !!event.ctrlKey !== usesCtrl) return false;
    } else {
      if (!event.metaKey && !event.ctrlKey) return false;
      if (usesCtrl && (!event.ctrlKey || !event.metaKey)) return false;
      if (usesMeta && (!event.metaKey || !event.ctrlKey)) return false;
    }
  } else {
    if (!!event.metaKey !== usesMeta) return false;
    if (!!event.ctrlKey !== usesCtrl) return false;
  }

  if (!!event.altKey !== modifiers.has("Alt")) return false;
  if (!!event.shiftKey !== modifiers.has("Shift")) return false;
  return matchesShortcutKey(event, key, platform);
}
