import type { Catalog, Group, Service, Specification, Tag } from "@/types/domain";
import { createId, UNGROUPED_GROUP_ID } from "@/lib/core";
import { specxApi } from "@/lib/backend/client";
import { translate } from "@/i18n";

export type CatalogBundle = { format: "specx-catalog"; version: 1; groups: Group[]; tags: Tag[]; services: Array<Pick<Service, "id" | "name" | "slug" | "description" | "groupId" | "tagIds">>; specifications: Array<Pick<Specification, "id" | "serviceId" | "title" | "content">> };

/** Portable content only. Authentication, validators and local UI state never travel. */
export function exportCatalog(catalog: Catalog): CatalogBundle {
  return {
    format: "specx-catalog",
    version: 1,
    groups: catalog.groups.filter((group) => group.id !== UNGROUPED_GROUP_ID).map(({ id, name, parentId, position }) => ({ id, name, parentId, position })),
    tags: catalog.tags.map(({ id, name, color }) => ({ id, name, color })),
    services: catalog.services.map(({ id, name, slug, description, groupId, tagIds }) => ({ id, name, slug, description, groupId, tagIds: [...tagIds] })),
    specifications: catalog.specifications.map(({ id, serviceId, title, content }) => ({ id, serviceId, title, content })),
  };
}

export function parseCatalogBundle(source: string): CatalogBundle {
  const invalid = () => {
    throw new Error(translate("transfer.invalidBackup"));
  };
  let value;
  try {
    value = JSON.parse(source);
  } catch {
    return invalid();
  }
  const text = (item: unknown): item is string => typeof item === "string" && item.trim().length > 0;
  if (!value || value.format !== "specx-catalog" || value.version !== 1) invalid();
  for (const key of ["groups", "tags", "services", "specifications"]) if (!Array.isArray(value[key]) || value[key].length > 5000) invalid();
  const ids = new Set<string>();
  for (const item of [...value.groups, ...value.tags, ...value.services, ...value.specifications]) {
    if (!item || !text(item.id) || ids.has(item.id)) invalid();
    ids.add(item.id);
  }
  const bundle = value as CatalogBundle;
  const groups = new Map(bundle.groups.map((group) => [group.id, group]));
  const tags = new Set(bundle.tags.map((tag) => tag.id));
  const services = new Set(bundle.services.map((service) => service.id));
  const slugs = new Set<string>();
  for (const group of bundle.groups) {
    if (!text(group.name) || !(group.parentId === null || groups.has(group.parentId)) || (group.position !== undefined && (!Number.isSafeInteger(group.position) || group.position < 0))) invalid();
    const seen = new Set<string>([group.id]);
    let parent = group.parentId;
    while (parent) {
      if (seen.has(parent)) invalid();
      seen.add(parent);
      parent = groups.get(parent)!.parentId;
    }
  }
  for (const tag of bundle.tags) if (!text(tag.name) || !/^#[0-9a-f]{6}$/i.test(tag.color)) invalid();
  for (const service of bundle.services) {
    if (!text(service.name) || !text(service.slug) || typeof service.description !== "string" || slugs.has(service.slug) || !(service.groupId === UNGROUPED_GROUP_ID || groups.has(service.groupId)) || !Array.isArray(service.tagIds) || service.tagIds.some((id) => !tags.has(id))) invalid();
    slugs.add(service.slug);
  }
  for (const spec of bundle.specifications) if (!text(spec.title) || !text(spec.content) || !services.has(spec.serviceId)) invalid();
  return bundle;
}

/** Preflight every record before sending writes. Imports add new resources; they
 * never overwrite an existing service with a matching slug. Each API write keeps
 * normal authorization/idempotency; progress explicitly reports partial failure. */
export async function importCatalog(bundle: CatalogBundle, catalog: Catalog, progress: (completed: number, total: number) => void) {
  const existingSlugs = new Set(catalog.services.map((service) => service.slug));
  const conflict = bundle.services.find((service) => existingSlugs.has(service.slug));
  if (conflict) throw new Error(translate("transfer.slugConflict", { slug: conflict.slug }));
  const ids = new Map([...bundle.groups, ...bundle.tags, ...bundle.services, ...bundle.specifications].map((item) => [item.id, createId("import")]));
  const total = ids.size;
  let completed = 0;
  const done = () => progress(++completed, total);
  const pending = new Map(bundle.groups.map((group) => [group.id, group]));
  while (pending.size) {
    const ready = [...pending.values()].filter((group) => !group.parentId || !pending.has(group.parentId));
    if (!ready.length) throw new Error(translate("transfer.cycle"));
    for (const group of ready) {
      await specxApi.createGroup(catalog.workspace.id, { id: ids.get(group.id)!, name: group.name, parentId: group.parentId ? ids.get(group.parentId)! : null, position: group.position });
      pending.delete(group.id);
      done();
    }
  }
  for (const tag of bundle.tags) {
    await specxApi.createTag(catalog.workspace.id, { ...tag, id: ids.get(tag.id)! });
    done();
  }
  for (const service of bundle.services) {
    await specxApi.createService(catalog.workspace.id, { ...service, id: ids.get(service.id)!, groupId: ids.get(service.groupId) ?? null, tagIds: service.tagIds.map((id) => ids.get(id)!) });
    done();
  }
  for (const spec of bundle.specifications) {
    await specxApi.createSpecification(catalog.workspace.id, { ...spec, id: ids.get(spec.id)!, serviceId: ids.get(spec.serviceId)! });
    done();
  }
}

export function downloadCatalog(catalog: Catalog) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(exportCatalog(catalog), null, 2)], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `specx-catalog-${new Date().toISOString().slice(0, 10)}.json`;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
