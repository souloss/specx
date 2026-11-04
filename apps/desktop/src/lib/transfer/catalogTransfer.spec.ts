import { beforeEach, describe, expect, it, vi } from "vitest";
import { exportCatalog, importCatalog, parseCatalogBundle } from "./catalogTransfer";
import { specxApi } from "@/lib/backend/client";
import { catalogFixture } from "../../test/catalogFixture";

vi.mock("@/lib/backend/client", () => ({ specxApi: { createGroup: vi.fn(), createTag: vi.fn(), createService: vi.fn(), createSpecification: vi.fn() } }));
beforeEach(() => vi.clearAllMocks());
describe("catalog transfers", () => {
  it("round-trips portable content without exporting validators or credentials", () => {
    const bundle = exportCatalog(catalogFixture());
    expect(parseCatalogBundle(JSON.stringify(bundle))).toEqual(bundle);
    expect(JSON.stringify(bundle)).not.toContain("etag");
    expect(JSON.stringify(bundle)).not.toContain("tenant");
  });
  it("reports malformed JSON as a localized backup validation failure", () => {
    expect(() => parseCatalogBundle("{invalid")).toThrow("文件不是有效的 specX 目录备份");
  });
  it.each(["cycle", "missing service", "duplicate slug", "invalid tag"])("rejects %s before making writes", (scenario) => {
    const bundle = exportCatalog(catalogFixture());
    if (scenario === "cycle") bundle.groups[0].parentId = bundle.groups[1].id;
    if (scenario === "missing service") bundle.specifications[0].serviceId = "missing";
    if (scenario === "duplicate slug") bundle.services[1].slug = bundle.services[0].slug;
    if (scenario === "invalid tag") bundle.services[0].tagIds.push("missing");
    expect(() => parseCatalogBundle(JSON.stringify(bundle))).toThrow();
    expect(specxApi.createGroup).not.toHaveBeenCalled();
  });
  it("preflights slug conflicts without writing partial groups", async () => {
    const catalog = catalogFixture();
    await expect(importCatalog(exportCatalog(catalog), catalog, vi.fn())).rejects.toThrow("已存在");
    expect(specxApi.createGroup).not.toHaveBeenCalled();
  });
  it("imports parents before children, remaps references, and reports persisted progress", async () => {
    const catalog = catalogFixture();
    const target = { ...catalog, groups: [], services: [], specifications: [], tags: [] };
    const progress = vi.fn();
    await importCatalog(exportCatalog(catalog), target, progress);
    const parent = vi.mocked(specxApi.createGroup).mock.calls[0][1];
    const child = vi.mocked(specxApi.createGroup).mock.calls[1][1];
    expect(child.parentId).toBe(parent.id);
    expect(vi.mocked(specxApi.createService).mock.calls[0][1].groupId).toBe(child.id);
    expect(vi.mocked(specxApi.createService).mock.calls[0][1].tagIds).toEqual([vi.mocked(specxApi.createTag).mock.calls[0][1].id]);
    expect(progress).toHaveBeenLastCalledWith(6, 6);
  });
});
