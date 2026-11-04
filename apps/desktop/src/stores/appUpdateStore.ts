import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { checkAppUpdate, getAppReleaseInfo, restartApp, type AppUpdate } from "@/lib/backend/appRelease";
import { buildInfo } from "@/lib/app/releases";
import { isTauriRuntime } from "@/lib/backend/runtime";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import type { MessageKey } from "@/i18n";
import { lockWritesForInstallation } from "@/lib/backend/writeActivity";

/** The store outlives the settings panel, so closing it cannot strand a download. */
export const useAppUpdateStore = defineStore("appUpdate", () => {
  const desktop = isTauriRuntime();
  const version = ref(buildInfo.version);
  const configured = ref(false);
  const initialized = ref(false);
  const phase = ref<"idle" | "checking" | "available" | "downloading" | "downloaded" | "installing" | "restart">("idle");
  const latestVersion = ref("");
  const errorKey = ref<MessageKey | null>(null);
  const checked = ref(false);
  const downloadedBytes = ref(0);
  const totalBytes = ref<number>();
  const autoCheck = ref(safeLocalStorageGet("specx-auto-check-updates") !== "false");
  watch(autoCheck, (value) => safeLocalStorageSet("specx-auto-check-updates", String(value)));
  const progress = computed(() => (totalBytes.value ? Math.min(100, Math.round((downloadedBytes.value * 100) / totalBytes.value)) : undefined));
  const busy = computed(() => ["checking", "downloading", "installing"].includes(phase.value));
  const available = computed(() => !!latestVersion.value);
  let update: AppUpdate | null = null;
  let initialization: Promise<void> | undefined;

  function initialize(): Promise<void> {
    initialization ??= (async () => {
      try {
        const info = await getAppReleaseInfo();
        version.value = info.version;
        configured.value = info.updaterConfigured;
        initialized.value = true;
      } catch {
        errorKey.value = "updates.checkFailed";
        initialization = undefined;
      }
    })();
    return initialization;
  }

  async function check() {
    await initialize();
    if (!desktop || !configured.value || busy.value || ["downloaded", "restart"].includes(phase.value)) return;
    phase.value = "checking";
    errorKey.value = null;
    try {
      const next = await checkAppUpdate();
      await update?.close().catch(() => {});
      update = next;
      latestVersion.value = next?.version ?? "";
      checked.value = true;
    } catch {
      errorKey.value = "updates.checkFailed";
    } finally {
      phase.value = update ? "available" : "idle";
    }
  }

  async function download() {
    if (!update || phase.value !== "available") return;
    phase.value = "downloading";
    errorKey.value = null;
    downloadedBytes.value = 0;
    totalBytes.value = undefined;
    try {
      await update.download(
        (event) => {
          if (event.event === "Started") totalBytes.value = event.data.contentLength;
          if (event.event === "Progress") downloadedBytes.value += event.data.chunkLength;
        },
        { timeout: 120000 },
      );
      // Finished can precede signature verification. Only the resolved download
      // promise is allowed to make the Install button available.
      phase.value = "downloaded";
    } catch {
      errorKey.value = "updates.downloadFailed";
      phase.value = "available";
    }
  }

  async function install() {
    if (!update || phase.value !== "downloaded") return;
    const unlock = lockWritesForInstallation();
    if (!unlock) {
      errorKey.value = "updates.pendingWork";
      return;
    }
    phase.value = "installing";
    errorKey.value = null;
    try {
      await update.install();
      phase.value = "restart";
      await restart();
    } catch {
      errorKey.value = "updates.installFailed";
      phase.value = "downloaded";
      unlock();
    }
  }

  async function restart() {
    if (phase.value !== "restart") return;
    errorKey.value = null;
    try {
      await restartApp();
    } catch {
      errorKey.value = "updates.restartFailed";
    }
  }

  return { desktop, version, configured, initialized, phase, latestVersion, errorKey, checked, autoCheck, progress, busy, available, initialize, check, download, install, restart };
});
