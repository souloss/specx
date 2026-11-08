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
  it("projects OpenAPI JSON into a specification, tag, and operation tree", () => {
    const catalog = catalogFixture();
    catalog.specifications.push({
      id: "openapi",
      serviceId: "svc-b",
      title: "Billing API",
      kind: "openapi",
      content: JSON.stringify({ openapi: "3.0.3", info: { title: "Billing", version: "1" }, paths: { "/invoices": { get: { tags: ["Invoices"], summary: "List invoices" } } } }),
      version: 1,
      contentHash: "sha256:openapi",
      updatedAt: "2026-11-03",
      etag: '"openapi-v1"',
    });
    const service = layoutTree(catalogSidebarLayout(catalog), catalog)[0]!.children![0]!.children!.find((node) => node.id === "svc-b")!;
    const provider = service.children!.find((node) => node.providerKind === "openapi")!;
    const specification = provider.children![0]!;
    expect(provider.label).toContain("OpenAPI");
    expect(specification.type).toBe("directory");
    expect(specification.children![0]!.label).toBe("Invoices");
    expect(specification.children![0]!.children![0]!.operationKey).toBe("GET:/invoices");
  });
  it("projects OpenAPI YAML into the same stable Tag and operation tree", () => {
    const catalog = catalogFixture();
    catalog.specifications.push({
      id: "yaml-api",
      serviceId: "svc-a",
      title: "Accounts API",
      kind: "openapi",
      content: "openapi: 3.1.0\ninfo:\n  title: Accounts\n  version: '1'\npaths:\n  /accounts:\n    post:\n      tags: [Accounts]\n      summary: Create account\n",
      version: 1,
      contentHash: "sha256:yaml",
      updatedAt: "2026-11-03",
      etag: '"yaml-v1"',
    });
    const service = layoutTree(catalogSidebarLayout(catalog), catalog)[0]!.children![0]!.children!.find((node) => node.id === "svc-a")!;
    const provider = service.children!.find((node) => node.providerKind === "openapi")!;
    const specification = provider.children!.find((node) => node.specificationId === "yaml-api")!;
    expect(specification.children![0]!.children![0]!.operationKey).toBe("POST:/accounts");
  });
  it("groups specifications from different providers under independent directories", () => {
    const catalog = catalogFixture();
    catalog.specifications.push({ ...catalog.specifications[0]!, id: "spec-2", title: "部署说明" });
    const service = layoutTree(catalogSidebarLayout(catalog), catalog)[0]!.children![0]!.children!.find((node) => node.id === "svc-a")!;
    expect(service.children).toHaveLength(1);
    expect(service.children?.[0]).toMatchObject({ providerKind: "markdown", providerRoot: true });
    expect(service.children?.[0]?.providerIcon).toBeTruthy();
    expect(service.children?.[0]?.label).toBe("Markdown");
    expect(service.children?.[0]?.children).toHaveLength(2);
    expect(service.children?.[0]?.children?.map((node) => node.specificationId)).toEqual(expect.arrayContaining(["spec", "spec-2"]));
    catalog.specifications.push({ ...catalog.specifications[0]!, id: "openapi-2", kind: "openapi", title: "Accounts API", content: JSON.stringify({ openapi: "3.0.3", info: { title: "Accounts", version: "1" }, paths: {} }) });
    const withOpenApi = layoutTree(catalogSidebarLayout(catalog), catalog)[0]!.children![0]!.children!.find((node) => node.id === "svc-a")!;
    expect(withOpenApi.children?.map((node) => node.providerKind).sort()).toEqual(["markdown", "openapi"]);
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
