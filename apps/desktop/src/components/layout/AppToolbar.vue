<script setup lang="ts">
import { computed } from "vue";
import { ChevronsRight, Plus, FilePlus2, Settings, Sun, Moon, SunMoon, LogOut } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { SpecxLogo } from "@/components/brand";
import LightDropdown from "@/components/ui/LightDropdown.vue";
import AppTooltip from "@/components/ui/AppTooltip.vue";
import { useTheme } from "@/composables/useTheme";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
import { useNavigationStore } from "@/stores/navigationStore";
import { useToast } from "@/composables/useToast";
import { isTauriRuntime } from "@/lib/backend/runtime";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useAppUpdateStore } from "@/stores/appUpdateStore";
import { translate } from "@/i18n";
import WindowControls from "@/components/layout/WindowControls.vue";
import { useWindowControls } from "@/composables/useWindowControls";
const emit = defineEmits<{ newService: []; newSpecification: []; logout: [] }>();
const settings = useSettingsStore();
const workspace = useWorkspaceStore();
const navigation = useNavigationStore();
const { toast } = useToast();
const { themeMode, setThemeMode, isDark } = useTheme();
const t = translate;
const shortcuts = useShortcutStore();
const updates = useAppUpdateStore();
const { isDesktop, isMac, isLinux, isMaximized, isFullscreen, toggleMaximize, toggleFullscreen, minimize, close, startDragging } = useWindowControls();
const themeIcon = computed(() => (themeMode.value === "system" ? SunMoon : isDark.value ? Moon : Sun));
function cycleTheme() {
  const next = isDark.value ? "light" : "dark";
  setThemeMode(next);
  toast(`${t("toolbar.switchTheme")}: ${t(`theme.${next}`)}`, 1600);
}
async function switchWorkspace(value: string) {
  if (navigation.busy || navigation.naming) return;
  try {
    await workspace.load(value);
  } catch (error) {
    toast(error instanceof Error ? error.message : t("theme.switchFailed"), 5000);
  }
}
function onToolbarPointerDown(event: PointerEvent) {
  if (!isDesktop || event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, a, [role='button'], [data-window-controls]")) return;
  void startDragging();
}
function onToolbarDoubleClick(event: MouseEvent) {
  if (!isDesktop) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, a, [role='button'], [data-window-controls]")) return;
  void toggleMaximize();
}
</script>

<template>
  <!-- Resource actions are grouped separately from application controls. -->
  <header class="app-toolbar h-10 flex items-center gap-1 border-b bg-muted/30 px-2 shrink-0" :class="{ 'app-toolbar-macos': isDesktop && isMac }" @pointerdown="onToolbarPointerDown" @dblclick="onToolbarDoubleClick">
    <div class="flex h-8 shrink-0 items-center px-2"><SpecxLogo :size="24" /></div>
    <AppTooltip v-if="settings.sidebarCollapsed" :text="shortcuts.hint(t('toolbar.expandSidebar'), 'sidebar')"
      ><Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="shortcuts.hint(t('sidebar.expand'), 'sidebar')" @click="settings.sidebarCollapsed = false"><ChevronsRight class="h-4 w-4" /></Button
    ></AppTooltip>
    <Button variant="ghost" size="sm" class="h-8 gap-1 px-2 text-xs" @click="emit('newService')"><Plus class="h-3.5 w-3.5" />{{ t("toolbar.createService") }}</Button>
    <Button variant="ghost" size="sm" class="h-8 gap-1 px-2 text-xs" :disabled="!workspace.activeService" @click="emit('newSpecification')"
      ><FilePlus2 class="h-3.5 w-3.5" /><span class="max-sm:hidden">{{ t("toolbar.createSpecification") }}</span></Button
    >
    <span class="mx-1 h-4 border-l border-border" />
    <LightDropdown
      :model-value="workspace.catalog?.workspace.id ?? ''"
      :disabled="navigation.busy || Boolean(navigation.naming)"
      :items="workspace.workspaces.map((item) => ({ value: item.id, label: item.name }))"
      :aria-label="t('toolbar.switchWorkspace')"
      trigger-class="inline-flex h-7 min-w-0 max-w-48 items-center gap-1 rounded px-2 text-xs hover:bg-muted"
      content-class="w-52"
      @update:model-value="switchWorkspace"
    />
    <div class="min-w-2 flex-1 self-stretch" />
    <AppTooltip :text="`${t('toolbar.switchTheme')}: ${t(`theme.${themeMode}`)}`">
      <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('toolbar.switchTheme')" @click="cycleTheme"><component :is="themeIcon" class="h-4 w-4" /></Button>
    </AppTooltip>
    <AppTooltip :text="shortcuts.hint(t('toolbar.settings'), 'settings')"
      ><Button variant="ghost" size="icon" class="relative h-8 w-8" :class="{ 'bg-accent': settings.showSettingsPage }" :aria-label="t('common.settings')" @click="settings.showSettingsPage = !settings.showSettingsPage"
        ><Settings class="h-4 w-4" /><span v-if="updates.available" class="absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-primary" :aria-label="t('updates.updateAvailable')" /></Button
    ></AppTooltip>
    <AppTooltip v-if="!isTauriRuntime()" :text="t('toolbar.logout')"
      ><Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('toolbar.logout')" @click="emit('logout')"><LogOut class="h-4 w-4" /></Button
    ></AppTooltip>
    <WindowControls v-if="isDesktop && !isMac" :is-linux="isLinux" :is-maximized="isMaximized" :is-fullscreen="isFullscreen" @minimize="minimize" @toggle-maximize="toggleMaximize" @toggle-fullscreen="toggleFullscreen" @close="close" />
  </header>
</template>

<style scoped>
.app-toolbar-macos {
  padding-left: 76px;
}
</style>
