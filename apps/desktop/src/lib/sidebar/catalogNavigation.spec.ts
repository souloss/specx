import { describe, expect, it } from "vitest";
import { catalogSidebarLayout, filterResourceTree, layoutPlacements, layoutTree } from "./catalogNavigation";
import { reorderEntry, collapseAllGroups, expandGroups } from "./sidebarLayout";
import { sortResourceListForDisplay } from "./resourceListSort";
import { catalogFixture } from "@/test/catalogFixture";

describe("catalog navigation adaptation", () => {
  it("preserves expansion while server hierarchy overrides stale local placement", () => {
    const catalog = catalogFixture();
    const old = collapseAllGroups(catalogSidebarLayout(catalog));
    catalog.services[0].groupId = "platform";
    const next = catalogSidebarLayout(catalog, old);
    expect(next.groups.every((group) => group.collapsed)).toBe(true);
    expect(layoutPlacements(next).resources.get("svc-b")).toBe("platform");
  });
  it("changes display sorting without losing manual placement", () => {
    const nodes = layoutTree(catalogSidebarLayout(catalogFixture()), catalogFixture());
    const sorted = sortResourceListForDisplay(nodes, "asc");
    expect(sorted[0].children![0].children!.map((node) => node.id)).toEqual(["svc-a", "svc-b"]);
    expect(sortResourceListForDisplay(nodes, "manual")[0].children![0].children!.map((node) => node.id)).toEqual(["svc-b", "svc-a"]);
  });
  it("keeps the ancestor path for matching slugs and combines tag filters", () => {
    const catalog = catalogFixture();
    const nodes = layoutTree(catalogSidebarLayout(catalog), catalog);
    const found = filterResourceTree(nodes, "billing", catalog, "critical");
    expect(found[0].id).toBe("platform");
    expect(found[0].children![0].children!.map((node) => node.id)).toEqual(["svc-b"]);
    expect(filterResourceTree(nodes, "accounts", catalog, "critical")).toEqual([]);
  });
  it("shows descendants when their group name matches", () => {
    const catalog = catalogFixture();
    const found = filterResourceTree(layoutTree(catalogSidebarLayout(catalog), catalog), "身份", catalog);
    expect(found[0].children![0].children).toHaveLength(2);
  });
  it("supports explicit regular expression matching", () => {
    const catalog = catalogFixture();
    const nodes = layoutTree(catalogSidebarLayout(catalog), catalog);
    const found = filterResourceTree(nodes, "^billing$", catalog, "", /^billing$/i);
    expect(found[0].children![0].children).toHaveLength(1);
  });
  it("rejects dragging a group into its own descendant without changing the source", () => {
    const layout = catalogSidebarLayout(catalogFixture());
    const snapshot = JSON.stringify(layout);
    expect(reorderEntry(layout, "platform", "auth", "inside")).toBe(layout);
    expect(JSON.stringify(layout)).toBe(snapshot);
  });
  it("collapses all groups and expands only the required location path", () => {
    const layout = collapseAllGroups(catalogSidebarLayout(catalogFixture()));
    const expanded = expandGroups(layout, ["platform", "auth"]);
    expect(expanded.groups.every((group) => !group.collapsed)).toBe(true);
    expect(layout.groups.every((group) => group.collapsed)).toBe(true);
  });
});
