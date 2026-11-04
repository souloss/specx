import { safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { APP_CORNER_STYLE_STORAGE_KEY, APP_CUSTOM_UI_DARK_STORAGE_KEY, APP_CUSTOM_UI_STORAGE_KEY, APP_THEME_PALETTE_STORAGE_KEY, APP_THEME_STORAGE_KEY, type AppCustomUiColors, type AppCornerStyle, type AppThemeMode, type AppThemePalette } from "./appTheme";

/** Renderer preferences deliberately use the same persistence on Web and Tauri.
 * Keep native appearance integration limited to APIs exposed by this host. */
export function persistAppAppearancePatch(patch: { themeMode?: AppThemeMode; themePalette?: AppThemePalette; cornerStyle?: AppCornerStyle; customUiColors?: AppCustomUiColors; customUiColorsDark?: AppCustomUiColors }) {
  if (patch.themeMode) safeLocalStorageSet(APP_THEME_STORAGE_KEY, patch.themeMode);
  if (patch.themePalette) safeLocalStorageSet(APP_THEME_PALETTE_STORAGE_KEY, patch.themePalette);
  if (patch.cornerStyle) safeLocalStorageSet(APP_CORNER_STYLE_STORAGE_KEY, patch.cornerStyle);
  if (patch.customUiColors) safeLocalStorageSet(APP_CUSTOM_UI_STORAGE_KEY, JSON.stringify(patch.customUiColors));
  if (patch.customUiColorsDark) safeLocalStorageSet(APP_CUSTOM_UI_DARK_STORAGE_KEY, JSON.stringify(patch.customUiColorsDark));
}
