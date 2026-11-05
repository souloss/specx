<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { Settings, Palette, FolderTree, Keyboard, Info, Search, X, Sun, Moon, SunMoon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import ShortcutSettings from "./ShortcutSettings.vue";
import AppReleasePanel from "./AppReleasePanel.vue";
import LightDropdown from "@/components/ui/LightDropdown.vue";
import { useTheme } from "@/composables/useTheme";
import { useSettingsStore } from "@/stores/settingsStore";
import { APP_THEME_PALETTES, APP_CUSTOM_UI_COLOR_DEFS } from "@/lib/app/appTheme";
import { customUiFontFamily, DEFAULT_UI_FONT_FAMILY, readableFontFamily, SYSTEM_UI_FONT_FAMILY, uiFontPresetFor, type UiFontPreset } from "@/lib/app/appFonts";
import { i18n, setLocale, translate } from "@/i18n";

const settings = useSettingsStore();
const t = translate;
const { themeMode, themePalette, isDark, activeCustomUiColors, cornerStyle, setThemeMode, setThemePalette, previewThemePalette, clearThemePalettePreview, setCustomUiColors, resetCustomUiColors, setCornerStyle } = useTheme();
const activeCategory = ref("appearance");
const search = ref("");
const categories = computed(() => [
  { id: "appearance", label: t("settings.appearance"), icon: Palette, keywords: t("settings.appearanceKeywords") },
  { id: "navigation", label: t("settings.navigation"), icon: FolderTree, keywords: t("settings.navigationKeywords") },
  { id: "shortcuts", label: t("settings.shortcuts"), icon: Keyboard, keywords: t("settings.shortcutsKeywords") },
  { id: "about", label: t("settings.about"), icon: Info, keywords: t("settings.aboutKeywords") },
]);
const visibleCategories = computed(() => categories.value.filter((category) => !search.value.trim() || `${category.label} ${category.keywords}`.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase())));
const current = computed(() => (search.value.trim() ? visibleCategories.value[0]?.id : activeCategory.value));
const uiFontPreset = ref<UiFontPreset>(uiFontPresetFor(settings.uiFontFamily));
const customFontName = ref(uiFontPreset.value === "custom" ? readableFontFamily(settings.uiFontFamily) : "");
const uiFontOptions = computed(
  () =>
    [
      { value: "default", label: t("settings.fontDefault") },
      { value: "system", label: t("settings.fontSystem") },
      { value: "custom", label: t("settings.fontCustom") },
    ] as const,
);
function selectUiFontPreset(value: unknown) {
  if (typeof value !== "string") return;
  if (value !== "default" && value !== "system" && value !== "custom") return;
  uiFontPreset.value = value;
  if (value === "default") settings.uiFontFamily = DEFAULT_UI_FONT_FAMILY;
  if (value === "system") settings.uiFontFamily = SYSTEM_UI_FONT_FAMILY;
  if (value === "custom" && customFontName.value) updateCustomFont(customFontName.value);
}
function updateCustomFont(value: string | number) {
  const name = String(value);
  customFontName.value = name;
  const fontFamily = customUiFontFamily(name);
  if (!fontFamily) return;
  uiFontPreset.value = "custom";
  settings.uiFontFamily = fontFamily;
}
const paletteNames = computed<Record<string, string>>(() => ({
  pearl: t("settings.palettePearl"),
  mist: t("settings.paletteMist"),
  graphite: t("settings.paletteGraphite"),
  cobalt: t("settings.paletteCobalt"),
  sage: t("settings.paletteSage"),
  amber: t("settings.paletteAmber"),
  blush: t("settings.paletteBlush"),
  vscode: "VS Code",
  idea: "IDEA",
  xcode: "Xcode",
  jetbrains: "JetBrains",
  cursor: "Cursor",
  claude: "Claude",
  custom: t("settings.paletteCustom"),
}));
const colorNames = computed<Record<string, string>>(() => ({ background: t("settings.colorBackground"), foreground: t("settings.colorForeground"), primary: t("settings.colorPrimary"), border: t("settings.colorBorder"), sidebar: t("settings.colorSidebar") }));
const themeModes = computed(
  () =>
    [
      { value: "light", label: t("theme.light"), icon: Sun },
      { value: "dark", label: t("theme.dark"), icon: Moon },
      { value: "system", label: t("theme.system"), icon: SunMoon },
    ] as const,
);
const cornerStyles = computed(
  () =>
    [
      { value: "none", label: t("settings.cornerNone"), radius: "0" },
      { value: "small", label: t("settings.cornerSmall"), radius: "3px" },
      { value: "large", label: t("settings.cornerLarge"), radius: "6px" },
    ] as const,
);
onBeforeUnmount(clearThemePalettePreview);
</script>

<template>
  <!-- Category rail, searchable content, and shared theme controls. -->
  <section class="settings-page flex h-full min-h-0 flex-col gap-4 overflow-hidden bg-background p-4" :aria-label="t('settings.ariaLabel')">
    <header class="flex shrink-0 items-center gap-2 text-base leading-none font-medium">
      <Settings class="h-4 w-4" />
      <h1>{{ t("settings.title") }}</h1>
    </header>
    <div class="settings-layout flex min-h-0 flex-1 flex-col gap-3 lg:flex-row">
      <nav class="settings-category-nav flex min-h-0 shrink-0 gap-1 overflow-x-auto border-b pb-3 lg:w-40 lg:flex-col lg:overflow-x-hidden lg:overflow-y-auto lg:border-b-0 lg:border-r lg:pb-0 lg:pr-3" :aria-label="t('settings.categoriesAriaLabel')">
        <button
          v-for="category in categories"
          :key="category.id"
          class="settings-category-button w-auto shrink-0 whitespace-nowrap rounded-md px-3 py-2 text-left text-sm transition-colors lg:w-full"
          :class="current === category.id ? 'bg-primary text-primary-foreground shadow-sm' : 'text-muted-foreground hover:bg-muted hover:text-foreground'"
          :aria-current="current === category.id ? 'page' : undefined"
          @click="
            activeCategory = category.id;
            search = '';
          "
        >
          {{ category.label }}
        </button>
      </nav>
      <div class="flex min-w-0 min-h-0 flex-1 flex-col pl-1">
        <div class="shrink-0 px-2 pt-1 pb-3">
          <div class="relative">
            <Search class="pointer-events-none absolute top-1/2 left-4 z-10 h-4 w-4 -translate-y-1/2 text-muted-foreground" /><Input
              v-model="search"
              :aria-label="t('settings.search')"
              :placeholder="t('settings.search')"
              class="h-11 w-full rounded-xl border-border bg-muted/30 pr-10 pl-11 text-sm shadow-none hover:bg-muted/40 focus-visible:ring-2 focus-visible:ring-inset"
            /><button v-if="search" class="absolute top-1/2 right-2 -translate-y-1/2 rounded-md p-1 text-muted-foreground hover:bg-muted" :aria-label="t('settings.clearSearch')" @click="search = ''"><X class="h-4 w-4" /></button>
          </div>
        </div>
        <div class="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-1 pr-2">
          <div v-if="search.trim() && visibleCategories.length > 1" class="mb-4 flex flex-wrap gap-2">
            <button
              v-for="category in visibleCategories"
              :key="category.id"
              class="rounded border border-border px-2 py-1 text-xs"
              @click="
                activeCategory = category.id;
                search = '';
              "
            >
              {{ category.label }}
            </button>
          </div>
          <div v-if="!current" class="py-10 text-center text-sm text-muted-foreground">{{ t("settings.noMatches") }}</div>
          <section v-else-if="current === 'appearance'" class="settings-appearance-section flex flex-col gap-4 py-2">
            <div class="settings-appearance-top-grid">
              <div class="settings-appearance-field min-w-0">
                <label for="color-palette" class="text-sm font-medium">{{ t("settings.palette") }}</label>
                <Select :model-value="themePalette" @update:model-value="(value) => setThemePalette(value as typeof themePalette)" @update:open="(open) => !open && clearThemePalettePreview()">
                  <SelectTrigger id="color-palette" class="h-8 w-full gap-1" :aria-label="t('settings.palette')"
                    ><SelectValue
                      ><span class="flex items-center gap-1"><span class="h-3 w-3 rounded-full border border-border" :style="{ background: APP_THEME_PALETTES.find((option) => option.value === themePalette)?.previewColor }" />{{ paletteNames[themePalette] }}</span></SelectValue
                    ></SelectTrigger
                  >
                  <SelectContent @pointerleave="clearThemePalettePreview"
                    ><SelectItem v-for="option in APP_THEME_PALETTES" :key="option.value" :value="option.value" @pointerenter="previewThemePalette(option.value)" @focus="previewThemePalette(option.value)"
                      ><span class="flex items-center gap-2"><span class="h-3 w-3 rounded-full border border-border" :style="{ background: option.previewColor }" />{{ paletteNames[option.value] }}</span></SelectItem
                    ></SelectContent
                  >
                </Select>
              </div>
              <div class="settings-appearance-field min-w-0">
                <label for="ui-font-family" class="text-sm font-medium">{{ t("settings.font") }}</label>
                <Select :model-value="uiFontPreset" @update:model-value="selectUiFontPreset">
                  <SelectTrigger id="ui-font-family" class="h-8 w-full" :aria-label="t('settings.font')"
                    ><SelectValue>{{ uiFontOptions.find((option) => option.value === uiFontPreset)?.label }}</SelectValue></SelectTrigger
                  >
                  <SelectContent>
                    <SelectItem v-for="option in uiFontOptions" :key="option.value" :value="option.value">{{ option.label }}</SelectItem>
                  </SelectContent>
                </Select>
                <Input v-if="uiFontPreset === 'custom'" :model-value="customFontName" class="mt-2 h-8" :aria-label="t('settings.fontCustomAria')" :placeholder="t('settings.fontCustomPlaceholder')" @update:model-value="updateCustomFont" />
              </div>
              <div class="settings-appearance-field min-w-0">
                <label for="app-language" class="text-sm font-medium">{{ t("settings.language") }}</label>
                <Select :model-value="i18n.global.locale.value" @update:model-value="(value) => typeof value === 'string' && ['zh-CN', 'en'].includes(value) && setLocale(value as 'zh-CN' | 'en')">
                  <SelectTrigger id="app-language" class="h-8 w-full" :aria-label="t('settings.language')"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="zh-CN">{{ t("settings.languageChinese") }}</SelectItem>
                    <SelectItem value="en">{{ t("settings.languageEnglish") }}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div class="settings-appearance-theme-grid">
              <div class="settings-appearance-group">
                <h2 class="text-sm font-medium">{{ t("settings.displayMode") }}</h2>
                <div class="flex flex-wrap gap-2">
                  <Button v-for="option in themeModes" :key="option.value" variant="outline" size="sm" class="h-8 gap-1.5 px-3" :class="{ 'specx-choice-selected': themeMode === option.value }" :aria-pressed="themeMode === option.value" @click="setThemeMode(option.value)"
                    ><component :is="option.icon" class="h-3.5 w-3.5" />{{ option.label }}</Button
                  >
                </div>
              </div>
              <div class="settings-appearance-group">
                <h2 class="text-sm font-medium">{{ t("settings.cornerStyle") }}</h2>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-for="option in cornerStyles"
                    :key="option.value"
                    variant="outline"
                    size="sm"
                    class="h-8 px-3"
                    :class="{ 'specx-choice-selected': cornerStyle === option.value }"
                    :style="{ borderRadius: option.radius }"
                    :aria-pressed="cornerStyle === option.value"
                    @click="setCornerStyle(option.value)"
                    >{{ option.label }}</Button
                  >
                </div>
              </div>
            </div>
            <div v-if="themePalette === 'custom'" class="rounded-lg border border-border p-4">
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-sm">{{ isDark ? t("settings.customDark") : t("settings.customLight") }}</h2>
                <Button variant="outline" size="sm" @click="resetCustomUiColors">{{ t("settings.resetColors") }}</Button>
              </div>
              <div class="grid grid-cols-2 gap-3">
                <label v-for="definition in APP_CUSTOM_UI_COLOR_DEFS" :key="definition.key" class="flex items-center justify-between text-xs"
                  >{{ colorNames[definition.key] }}<input type="color" :aria-label="colorNames[definition.key]" :value="activeCustomUiColors[definition.key]" @input="setCustomUiColors({ ...activeCustomUiColors, [definition.key]: ($event.target as HTMLInputElement).value })"
                /></label>
              </div>
            </div>
            <div class="border-t border-border" />
            <div class="settings-appearance-group">
              <h2 class="text-sm font-medium">{{ t("settings.layout") }}</h2>
              <div class="settings-appearance-choice-grid">
                <Button variant="outline" class="h-auto justify-start border p-3" :class="{ 'specx-choice-selected': settings.appLayout === 'separated' }" :aria-pressed="settings.appLayout === 'separated'" @click="settings.appLayout = 'separated'"
                  ><span class="min-w-0 text-left"
                    ><span class="block text-sm font-medium">{{ t("settings.layoutSeparated") }}</span
                    ><span class="block whitespace-normal text-xs text-muted-foreground">{{ t("settings.layoutSeparatedDescription") }}</span></span
                  ></Button
                ><Button variant="outline" class="h-auto justify-start border p-3" :class="{ 'specx-choice-selected': settings.appLayout === 'classic' }" :aria-pressed="settings.appLayout === 'classic'" @click="settings.appLayout = 'classic'"
                  ><span class="min-w-0 text-left"
                    ><span class="block text-sm font-medium">{{ t("settings.layoutClassic") }}</span
                    ><span class="block whitespace-normal text-xs text-muted-foreground">{{ t("settings.layoutClassicDescription") }}</span></span
                  ></Button
                >
              </div>
            </div>
          </section>
          <section v-else-if="current === 'navigation'" class="space-y-5 py-2">
            <h2 class="text-sm font-medium">{{ t("settings.navigation") }}</h2>
            <label class="settings-field max-w-xs"
              >{{ t("settings.defaultSort")
              }}<LightDropdown
                :model-value="settings.navigation.sortMode"
                :items="[
                  { value: 'manual', label: t('settings.sortManual') },
                  { value: 'asc', label: t('settings.sortAsc') },
                  { value: 'desc', label: t('settings.sortDesc') },
                ]"
                :aria-label="t('settings.defaultSort')"
                trigger-class="settings-select"
                @update:model-value="(value) => (settings.navigation.sortMode = value as 'manual' | 'asc' | 'desc')" /></label
            ><label class="flex items-center gap-2 text-sm"><input v-model="settings.navigation.compact" type="checkbox" />{{ t("settings.compactRows") }}</label
            ><label class="flex items-center gap-2 text-sm"><input v-model="settings.navigation.showDescriptions" type="checkbox" />{{ t("settings.showDescriptions") }}</label>
            <p class="text-xs text-muted-foreground">{{ t("settings.navigationStorage") }}</p>
            <Button variant="outline" size="sm" @click="settings.resetNavigation">{{ t("settings.resetNavigation") }}</Button>
          </section>
          <ShortcutSettings v-else-if="current === 'shortcuts'" />
          <AppReleasePanel v-else />
        </div>
      </div>
    </div>
  </section>
</template>
