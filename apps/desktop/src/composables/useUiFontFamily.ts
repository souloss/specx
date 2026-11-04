import { watch } from "vue";
import { useSettingsStore } from "@/stores/settingsStore";
import { APP_FONT_SANS_CSS_VAR, DEFAULT_UI_FONT_FAMILY } from "@/lib/app/appFonts";
import { useTheme } from "@/composables/useTheme";

export function useUiFontFamily() {
  const settings = useSettingsStore();
  const { writeRootToken } = useTheme();
  watch(
    () => settings.uiFontFamily,
    (fontFamily) => writeRootToken(APP_FONT_SANS_CSS_VAR, fontFamily || DEFAULT_UI_FONT_FAMILY),
    { immediate: true },
  );
}
