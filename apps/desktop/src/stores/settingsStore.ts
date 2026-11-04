import { ref, watch } from "vue";
import { defineStore } from "pinia";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { DEFAULT_UI_FONT_FAMILY, UI_FONT_FAMILY_STORAGE_KEY } from "@/lib/app/appFonts";
import type { ResourceListSortMode } from "@/lib/sidebar/resourceListSort";

export type NavigationPreferences = { sortMode: ResourceListSortMode; showDescriptions: boolean; compact: boolean };
const defaults: NavigationPreferences = { sortMode: "manual", showDescriptions: true, compact: true };

/** UI preferences are independent of the shared catalog and never mutate service data. */
export const useSettingsStore = defineStore("settings", () => {
  const navigation = ref<NavigationPreferences>({ ...defaults });
  const sidebarCollapsed = ref(safeLocalStorageGet("specx-sidebar-collapsed") === "true");
  const showSettingsPage = ref(false);
  const appLayout = ref(safeLocalStorageGet("specx-app-layout") === "separated" ? "separated" : "classic");
  const uiFontFamily = ref(safeLocalStorageGet(UI_FONT_FAMILY_STORAGE_KEY) || DEFAULT_UI_FONT_FAMILY);
  try {
    const saved = JSON.parse(safeLocalStorageGet("specx-navigation-settings") ?? "{}");
    navigation.value = {
      sortMode: ["manual", "asc", "desc"].includes(saved.sortMode) ? saved.sortMode : defaults.sortMode,
      showDescriptions: typeof saved.showDescriptions === "boolean" ? saved.showDescriptions : defaults.showDescriptions,
      compact: typeof saved.compact === "boolean" ? saved.compact : defaults.compact,
    };
  } catch {
    /* A damaged preference must not prevent startup. */
  }
  watch(navigation, (value) => safeLocalStorageSet("specx-navigation-settings", JSON.stringify(value)), { deep: true });
  watch(sidebarCollapsed, (value) => safeLocalStorageSet("specx-sidebar-collapsed", String(value)));
  watch(appLayout, (value) => safeLocalStorageSet("specx-app-layout", value));
  watch(uiFontFamily, (value) => safeLocalStorageSet(UI_FONT_FAMILY_STORAGE_KEY, value));
  function resetNavigation() {
    navigation.value = { ...defaults };
  }
  return { navigation, sidebarCollapsed, showSettingsPage, appLayout, uiFontFamily, resetNavigation };
});
