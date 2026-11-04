import { describe, expect, it } from "vitest";
import { displayGroupName, flattenGroupIds, formatDate, groupPath, UNGROUPED_GROUP_ID } from "./core";
import { setLocale } from "@/i18n";
import type { Group } from "../types/domain";

const groups: Group[] = [
  { id: "root", name: "Services", parentId: null },
  { id: "platform", name: "Platform", parentId: "root" },
  { id: "internal", name: "Internal", parentId: "platform" },
];

describe("workspace directory helpers", () => {
  it("resolves nested group paths and descendants", () => {
    expect(groupPath(groups, "internal")).toEqual(["Services", "Platform", "Internal"]);
    expect(flattenGroupIds(groups, "root")).toEqual(["root", "platform", "internal"]);
  });

  it("localizes display-only names and dates without changing workspace data", () => {
    setLocale("en");
    expect(displayGroupName(UNGROUPED_GROUP_ID, "未归类")).toBe("Uncategorized");
    expect(formatDate("2026-11-03")).toBe("Nov 3, 2026");

    setLocale("zh-CN");
    expect(displayGroupName(UNGROUPED_GROUP_ID, "Uncategorized")).toBe("未归类");
    expect(formatDate("2026-11-03")).toContain("2026");
  });
});
