import zhCN from "../../../../../releases/changelog.zh-CN.json";
import en from "../../../../../releases/changelog.en.json";
import type { AppLocale } from "@/i18n";

export type BuildInfo = { version: string; repository: string; commit: string };
declare const __SPECX_BUILD_INFO__: BuildInfo;
export const buildInfo = __SPECX_BUILD_INFO__;
export type ChangelogItem = { title: string; desc: string };
export type ChangelogSection = { type: string; title: string; items: ChangelogItem[] };
export type ChangelogRelease = { tag: string; name: string; date: string; sections: ChangelogSection[] };
export type ChangelogData = { releases: ChangelogRelease[] };

export function bundledChangelog(locale: AppLocale): ChangelogData {
  return locale === "zh-CN" ? zhCN : en;
}

/** Remote notes are untrusted data. Render their strings as text, never HTML. */
export function parseChangelog(value: unknown): ChangelogData {
  if (!value || typeof value !== "object" || !Array.isArray((value as ChangelogData).releases)) throw new Error("invalid_changelog");
  const releases = (value as ChangelogData).releases;
  if (
    releases.length > 200 ||
    releases.some(
      (release) =>
        !release ||
        !/^v\d+\.\d+\.\d+$/.test(release.tag) ||
        typeof release.name !== "string" ||
        typeof release.date !== "string" ||
        !Number.isFinite(Date.parse(release.date)) ||
        !Array.isArray(release.sections) ||
        release.sections.some((section) => !section || typeof section.type !== "string" || typeof section.title !== "string" || !Array.isArray(section.items) || section.items.some((item) => !item || typeof item.title !== "string" || typeof item.desc !== "string")),
    )
  )
    throw new Error("invalid_changelog");
  return { releases };
}

export function releasesUrl(tag?: string): string | undefined {
  const repository = buildInfo.repository;
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) return undefined;
  return `https://github.com/${repository}/releases${tag ? `/tag/${encodeURIComponent(tag)}` : ""}`;
}
