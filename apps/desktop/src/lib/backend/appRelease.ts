import { isTauriRuntime } from "./runtime";
import { buildInfo, type ChangelogData, parseChangelog } from "@/lib/app/releases";
import type { AppLocale } from "@/i18n";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";

export type AppReleaseInfo = { version: string; updaterConfigured: boolean };
export type AppUpdate = {
  version: string;
  date?: string;
  download(onEvent: (event: DownloadEvent) => void, options: { timeout: number }): Promise<void>;
  install(): Promise<void>;
  close(): Promise<void>;
};

export async function getAppReleaseInfo(): Promise<AppReleaseInfo> {
  if (!isTauriRuntime()) return { version: buildInfo.version, updaterConfigured: false };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<AppReleaseInfo>("get_app_release_info");
}

export async function checkAppUpdate(): Promise<AppUpdate | null> {
  if (!isTauriRuntime()) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  return check({ timeout: 15000 });
}

export async function fetchAppChangelog(locale: AppLocale): Promise<ChangelogData> {
  const { invoke } = await import("@tauri-apps/api/core");
  return parseChangelog(await invoke("fetch_app_changelog", { locale }));
}

export async function restartApp(): Promise<void> {
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
