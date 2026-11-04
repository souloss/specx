import { onBeforeUnmount, onMounted } from "vue";
import { SHORTCUT_ACTIONS } from "@/lib/editor/shortcutRegistry";
import { useShortcutStore } from "@/stores/shortcutStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useWorkspaceStore } from "@/stores/workspace";
import { useNavigationStore } from "@/stores/navigationStore";

/** Register once at the application boundary. Tree editing keeps its own keys. */
export function useAppShortcuts(focusSearch: () => void) {
  const shortcuts = useShortcutStore();
  const settings = useSettingsStore();
  const workspace = useWorkspaceStore();
  const navigation = useNavigationStore();
  function handle(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing || event.repeat || !workspace.authenticated) return;
    if (document.querySelector('[data-slot="dialog-content"][data-state="open"]')) return;
    if (event.target instanceof Element && event.target.closest("[data-shortcut-recorder]")) return;
    const action = SHORTCUT_ACTIONS.find((item) => item.scope === "global" && shortcuts.matches(item.id, event))?.id;
    if (!event.ctrlKey && !event.metaKey && !event.altKey && event.target instanceof Element && event.target.closest('input, textarea, [contenteditable="true"]')) return;
    if (action === "settings") settings.showSettingsPage = !settings.showSettingsPage;
    else if (action === "sidebar") settings.sidebarCollapsed = !settings.sidebarCollapsed;
    else if (action === "search") {
      settings.sidebarCollapsed = false;
      focusSearch();
    } else if (action === "locate") {
      settings.sidebarCollapsed = false;
      navigation.locateActive();
    } else if (action === "newService") {
      settings.sidebarCollapsed = false;
      navigation.beginCreate("resource");
    } else if (action === "closeTab") {
      if (settings.showSettingsPage) settings.showSettingsPage = false;
      else if (workspace.activeTabId) workspace.closeTab(workspace.activeTabId);
    } else return;
    event.preventDefault();
  }
  onMounted(() => window.addEventListener("keydown", handle));
  onBeforeUnmount(() => window.removeEventListener("keydown", handle));
}
