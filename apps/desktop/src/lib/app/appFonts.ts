/* Adapted from Apache-2.0-licensed upstream code; see THIRD_PARTY_NOTICES.md. */

export const APP_FONT_SANS_CSS_VAR = "--font-sans";
export const UI_FONT_FAMILY_STORAGE_KEY = "specx-ui-font-family";
export const DEFAULT_UI_FONT_FAMILY = '"Geist Variable", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Segoe UI", system-ui, sans-serif';
export const SYSTEM_UI_FONT_FAMILY = 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';

export type UiFontPreset = "default" | "system" | "custom";

export function uiFontPresetFor(fontFamily: string): UiFontPreset {
  if (fontFamily === DEFAULT_UI_FONT_FAMILY) return "default";
  if (fontFamily === SYSTEM_UI_FONT_FAMILY) return "system";
  return "custom";
}

export function readableFontFamily(fontFamily: string): string {
  const firstFamily = fontFamily.split(",")[0]?.trim() ?? fontFamily;
  return firstFamily.replace(/^['"]|['"]$/g, "");
}

export function customUiFontFamily(name: string): string | null {
  const family = name.trim().replace(/^(['"])(.*)\1$/, "$2");
  if (!family || /[\u0000-\u001f;{}]/.test(family)) return null;
  const quoted = family.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return `"${quoted}", ${DEFAULT_UI_FONT_FAMILY}`;
}
