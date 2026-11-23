import { getOpenApiOperations, isRecord, type OpenApiObject } from "./openapi";
import { openApiPointer, openApiPointerParts, openApiPointerValue, visitOpenApiReferences } from "./openapiReferences";

export type OpenApiModel = { name: string; reference: string; schema: OpenApiObject | boolean; description: string };
export type OpenApiModelUsage = { kind: "operation" | "model"; key: string; title: string; direct: boolean };
export type OpenApiModelChange = { kind: "model"; previousReference?: string; reference?: string; title?: string };

export function getOpenApiModels(document: OpenApiObject | null): OpenApiModel[] {
  if (!document) return [];
  return [["components", "schemas"], ["definitions"]]
    .flatMap((parts) => {
      const values = openApiPointerValue(document, openApiPointer(parts));
      return isRecord(values)
        ? Object.entries(values).flatMap(([name, schema]) => (isRecord(schema) || typeof schema === "boolean" ? [{ name, reference: openApiPointer([...parts, name]), schema, description: isRecord(schema) && typeof schema.description === "string" ? schema.description : "" }] : []))
        : [];
    })
    .sort((left, right) => left.name.localeCompare(right.name));
}

/** A reference to a model's property opens the owning model. */
export function openApiModelForReference(document: OpenApiObject | null, reference: string): OpenApiModel | undefined {
  const parts = openApiPointerParts(reference);
  if (!parts) return undefined;
  const length = parts[0] === "components" && parts[1] === "schemas" ? 3 : parts[0] === "definitions" ? 2 : 0;
  if (!length || parts.length < length) return undefined;
  const owner = openApiPointer(parts.slice(0, length));
  const schema = document ? openApiPointerValue(document, owner) : undefined;
  if (!isRecord(schema) && typeof schema !== "boolean") return undefined;
  return { name: parts[length - 1]!, reference: owner, schema, description: isRecord(schema) && typeof schema.description === "string" ? schema.description : "" };
}

function modelReferences(document: OpenApiObject, value: unknown, models: OpenApiModel[]): Set<string> {
  const found = new Set<string>();
  const visited = new Set<string>();
  function scan(source: unknown) {
    visitOpenApiReferences(source, (reference, path) => {
      const model = openApiModelForReference(document, reference) ?? (path.at(-2) === "mapping" && path.at(-3) === "discriminator" ? models.find((candidate) => candidate.name === reference) : undefined);
      if (model) found.add(model.reference);
      else if (openApiPointerParts(reference) && !visited.has(reference)) {
        visited.add(reference);
        scan(openApiPointerValue(document, reference));
      }
    });
  }
  scan(value);
  return found;
}

export function getOpenApiModelDependencies(document: OpenApiObject, reference: string): OpenApiModel[] {
  const model = openApiModelForReference(document, reference);
  if (!model) return [];
  const models = getOpenApiModels(document);
  const references = modelReferences(document, model.schema, models);
  return models.filter((item) => item.reference !== reference && references.has(item.reference));
}

export function getOpenApiModelUsages(document: OpenApiObject, reference: string): OpenApiModelUsage[] {
  const models = getOpenApiModels(document);
  const dependencies = new Map(models.map((model) => [model.reference, modelReferences(document, model.schema, models)]));
  function reaches(refs: Set<string>, visited = new Set<string>()): boolean {
    if (refs.has(reference)) return true;
    for (const ref of refs) {
      if (visited.has(ref)) continue;
      visited.add(ref);
      if (reaches(dependencies.get(ref) ?? new Set(), visited)) return true;
    }
    return false;
  }
  const usages: OpenApiModelUsage[] = [];
  for (const model of models) {
    if (model.reference === reference) continue;
    const refs = dependencies.get(model.reference)!;
    if (reaches(refs)) usages.push({ kind: "model", key: model.reference, title: model.name, direct: refs.has(reference) });
  }
  for (const entry of getOpenApiOperations(JSON.stringify(document))) {
    const paths = isRecord(document.paths) ? document.paths : {};
    const rawPath = paths[entry.path];
    const pathItem = isRecord(rawPath) ? rawPath : {};
    const refs = modelReferences(document, { operation: entry.operation, parameters: pathItem.parameters }, models);
    if (reaches(refs)) usages.push({ kind: "operation", key: entry.identityKey, title: `${entry.method.toUpperCase()} ${entry.path}`, direct: refs.has(reference) });
  }
  return usages;
}

export function validateOpenApiModelName(document: OpenApiObject, name: string, currentReference?: string): "invalidName" | "duplicate" | null {
  if (!/^[A-Za-z0-9._-]+$/.test(name.trim())) return "invalidName";
  const current = currentReference ? openApiModelForReference(document, currentReference) : undefined;
  const parts = current ? openApiPointerParts(current.reference)!.slice(0, -1) : ["components", "schemas"];
  const nextRef = openApiPointer([...parts, name.trim()]);
  return nextRef !== currentReference && openApiPointerValue(document, nextRef) !== undefined ? "duplicate" : null;
}

function clone(document: OpenApiObject): OpenApiObject {
  return JSON.parse(JSON.stringify(document)) as OpenApiObject;
}

export function updateOpenApiModel(document: OpenApiObject, reference: string | undefined, name: string, schema: unknown): OpenApiObject {
  const invalid = validateOpenApiModelName(document, name, reference);
  if (invalid) throw new Error(invalid);
  if (!(isRecord(schema) || (typeof schema === "boolean" && typeof document.openapi === "string" && document.openapi.startsWith("3.1")))) throw new Error("invalidSchema");
  if (reference && !getOpenApiModels(document).some((model) => model.reference === reference)) throw new Error("missing");
  const next = clone(document);
  const parts = reference ? openApiPointerParts(reference)! : ["components", "schemas", name.trim()];
  let parent: OpenApiObject = next;
  for (const key of parts.slice(0, -1)) {
    if (!isRecord(parent[key])) parent[key] = {};
    parent = parent[key] as OpenApiObject;
  }
  delete parent[parts.at(-1)!];
  // defineProperty also treats user input such as `__proto__` as an own JSON key.
  const copiedSchema: unknown = JSON.parse(JSON.stringify(schema));
  Object.defineProperty(parent, name.trim(), { value: copiedSchema, enumerable: true, configurable: true, writable: true });
  const renamed = openApiPointer([...parts.slice(0, -1), name.trim()]);
  if (reference && reference !== renamed)
    visitOpenApiReferences(next, (value) => {
      const refParts = openApiPointerParts(value);
      if (refParts && parts.every((part, index) => refParts[index] === part)) return openApiPointer([...parts.slice(0, -1), name.trim(), ...refParts.slice(parts.length)]);
      // OpenAPI discriminator mapping may use a bare schema name.
      return undefined;
    });
  if (reference && reference !== renamed) {
    // Bare mappings are model names, whereas examples carrying those names stay literal.
    visitOpenApiReferences(next, (value, path) => (path.at(-2) === "mapping" && path.at(-3) === "discriminator" && value === parts.at(-1) ? name.trim() : undefined));
  }
  return next;
}

export function openApiModelDeletionBlockers(document: OpenApiObject, reference: string): string[] {
  const next = clone(document);
  const parts = openApiPointerParts(reference);
  if (!parts) return [];
  const parent = openApiPointerValue(next, openApiPointer(parts.slice(0, -1)));
  if (isRecord(parent)) delete parent[parts.at(-1)!];
  const blockers: string[] = [];
  visitOpenApiReferences(next, (value, path) => {
    const refParts = openApiPointerParts(value);
    if ((refParts && parts.every((part, index) => refParts[index] === part)) || (path.at(-2) === "mapping" && path.at(-3) === "discriminator" && value === parts.at(-1))) blockers.push(openApiPointer(path));
  });
  return blockers;
}

export function deleteOpenApiModel(document: OpenApiObject, reference: string): OpenApiObject {
  if (openApiModelDeletionBlockers(document, reference).length) throw new Error("inUse");
  const next = clone(document);
  const parts = openApiPointerParts(reference);
  if (!parts) throw new Error("missing");
  const parent = openApiPointerValue(next, openApiPointer(parts.slice(0, -1)));
  if (!isRecord(parent) || !Object.hasOwn(parent, parts.at(-1)!)) throw new Error("missing");
  delete parent[parts.at(-1)!];
  return next;
}
