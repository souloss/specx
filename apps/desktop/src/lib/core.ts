import { uuid } from "./common/utils";
import { i18n, translate } from "@/i18n";
import type { WorkspaceResponse } from "../types/api";
import type { Group, PanelTab, Service } from "../types/domain";

export const UNGROUPED_GROUP_ID = "__specx_ungrouped__";

export function createId(prefix: string): string {
  return `${prefix}-${uuid()}`;
}

export function groupPath(groups: Group[], groupId: string): string[] {
  const names: string[] = [];
  const visited = new Set<string>();
  let current = groups.find((group) => group.id === groupId);
  while (current && !visited.has(current.id)) {
    visited.add(current.id);
    names.unshift(displayGroupName(current.id, current.name));
    current = current.parentId ? groups.find((group) => group.id === current!.parentId) : undefined;
  }
  return names;
}

export function flattenGroupIds(groups: Group[], rootId: string): string[] {
  const ids = [rootId];
  for (let index = 0; index < ids.length; index += 1) {
    ids.push(...groups.filter((group) => group.parentId === ids[index]).map((group) => group.id));
  }
  return ids;
}

export function formatDate(value: string, locale: "zh-CN" | "en" = i18n.global.locale.value as "zh-CN" | "en"): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? translate("common.unknown") : new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(date);
}

export function displayGroupName(id: string, name: string): string {
  return id === UNGROUPED_GROUP_ID ? translate("common.ungrouped") : name;
}

export function overviewTab(serviceId: string): PanelTab {
  return { id: `overview-${serviceId}`, kind: "overview", serviceId, title: translate("service.overview") };
}

export function loadPanelSession(workspaceId: string, services: Service[], specs: { id: string; serviceId: string }[]) {
  try {
    const value = localStorage.getItem(`specx-panel-session-v1:${workspaceId}`);
    if (value) {
      const saved = JSON.parse(value) as { tabs?: PanelTab[]; activeTabId?: string };
      const tabs = (saved.tabs ?? []).filter((tab) => services.some((service) => service.id === tab.serviceId) && (tab.kind !== "specification" || specs.some((spec) => spec.id === tab.specificationId)));
      if (Array.isArray(saved.tabs)) {
        const activeTabId = tabs.some((tab) => tab.id === saved.activeTabId) ? saved.activeTabId! : (tabs[0]?.id ?? "");
        return { tabs, activeTabId };
      }
    }
  } catch {
    // Use a fresh overview when storage is unavailable or stale.
  }
  const serviceId = services[0]?.id;
  if (!serviceId) return { tabs: [], activeTabId: "" };
  const tab = { ...overviewTab(serviceId), title: services[0].name };
  return { tabs: [tab], activeTabId: tab.id };
}

export function resolveWorkspace(items: WorkspaceResponse[], workspaceId?: string) {
  return workspaceId ? items.find((item) => item.id === workspaceId) : items[0];
}
