import { onMounted, onUnmounted, ref } from "vue";
import { isTauriRuntime } from "@/lib/backend/runtime";

/** Native window actions stay behind this small runtime-neutral boundary. */
export function useWindowControls() {
  const desktop = isTauriRuntime();
  const isMaximized = ref(false);
  const isFullscreen = ref(false);
  let unlistenResize: (() => void) | undefined;
  let currentWindow: Awaited<ReturnType<(typeof import("@tauri-apps/api/window"))["getCurrentWindow"]>> | undefined;

  async function loadWindow() {
    if (!desktop) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    currentWindow ??= getCurrentWindow();
    return currentWindow;
  }
  async function refresh() {
    const window = await loadWindow();
    if (!window) return;
    [isMaximized.value, isFullscreen.value] = await Promise.all([window.isMaximized(), window.isFullscreen()]);
  }
  async function minimize() {
    await (await loadWindow())?.minimize();
  }
  async function toggleMaximize() {
    const window = await loadWindow();
    if (!window) return;
    await window.toggleMaximize();
    await refresh();
  }
  async function close() {
    await (await loadWindow())?.close();
  }
  async function startDragging() {
    await (await loadWindow())?.startDragging();
  }
  onMounted(async () => {
    const window = await loadWindow();
    if (!window) return;
    await refresh();
    unlistenResize = await window.onResized(() => void refresh());
  });
  onUnmounted(() => unlistenResize?.());
  return { isDesktop: desktop, isMaximized, isFullscreen, minimize, toggleMaximize, close, startDragging };
}
