import { visitOpenApiReferences } from "../contract/openapiReferences";
import { OPENAPI_METHODS, isRecord, resolveOpenApiRef, type OpenApiMethod, type OpenApiObject } from "../contract/openapi";

export type OpenApiOperationAddress = { method: string; path: string };
export type OpenApiOperationChange = { previousKey?: string; key: string; title: string };
export type OpenApiOperationAddressError = "method" | "path" | "duplicate" | "template" | "reference" | "missing";

function address(key: string): OpenApiOperationAddress {
  const separator = key.indexOf(":");
  return { method: key.slice(0, separator).toLowerCase(), path: key.slice(separator + 1) };
}

function pathItem(document: OpenApiObject, path: string): OpenApiObject | null {
  return isRecord(document.paths) && Object.hasOwn(document.paths, path) && isRecord(document.paths[path]) ? document.paths[path] : null;
}

function pathNames(path: string): string[] {
  return [...path.matchAll(/\{([^{}]+)\}/gu)].map((match) => match[1]!);
}

export function operationPathParameterRenames(previousPath: string, path: string) {
  const oldNames = pathNames(previousPath);
  const names = pathNames(path);
  const unmatchedOld = oldNames.filter((name) => !names.includes(name));
  const unmatchedNew = names.filter((name) => !oldNames.includes(name));
  return new Map(names.length === oldNames.length ? unmatchedOld.map((name, index) => [name, unmatchedNew[index]!]) : []);
}

export function validateOpenApiOperationAddress(document: OpenApiObject, input: OpenApiOperationAddress, previousKey?: string): OpenApiOperationAddressError | null {
  const method = input.method.toLowerCase();
  const path = input.path.trim();
  if (!OPENAPI_METHODS.includes(method as OpenApiMethod)) return "method";
  const names = pathNames(path);
  const hasControlCharacter = [...path].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127);
  if (!path.startsWith("/") || hasControlCharacter || /[\s?#\\]/u.test(path) || /[{}]/u.test(path.replace(/\{[^{}/]+\}/gu, "")) || new Set(names).size !== names.length) return "path";
  const previous = previousKey ? address(previousKey) : null;
  const source = previous ? pathItem(document, previous.path) : null;
  if (previous && (!source || !isRecord(source[previous.method]))) return "missing";
  const target = pathItem(document, path);
  if (target?.$ref || source?.$ref) return "reference";
  if (target && Object.hasOwn(target, method) && !(previous?.path === path && previous.method === method)) return "duplicate";
  const shape = path.replace(/\{[^{}]+\}/gu, "{}");
  for (const existing of Object.keys(isRecord(document.paths) ? document.paths : {})) {
    if (existing === path || existing.replace(/\{[^{}]+\}/gu, "{}") !== shape) continue;
    // A source path containing only this operation disappears after the move.
    if (existing === previous?.path && source && Object.keys(source).every((key) => key === previous.method)) continue;
    return "template";
  }
  return null;
}

function clone(document: OpenApiObject): OpenApiObject {
  return JSON.parse(JSON.stringify(document)) as OpenApiObject;
}

function parameterKey(document: OpenApiObject, raw: unknown): string | null {
  const parameter = resolveOpenApiRef(document, raw);
  return typeof parameter?.name === "string" && typeof parameter.in === "string" ? `${parameter.in}:${parameter.name}` : null;
}

function matchPathParameters(document: OpenApiObject, operation: OpenApiObject, target: OpenApiObject, path: string, previousPath?: string, inherited: unknown[] = []) {
  const names = pathNames(path);
  const renamed = previousPath ? operationPathParameterRenames(previousPath, path) : new Map<string, string>();
  const local = Array.isArray(operation.parameters) ? operation.parameters : [];
  const localKeys = new Set(local.map((raw) => parameterKey(document, raw)).filter(Boolean));
  const combined = [...local.map((raw, index) => ({ raw, index })), ...inherited.filter((raw) => !localKeys.has(parameterKey(document, raw))).map((raw) => ({ raw, index: -1 }))];
  const indexes = new Map<number, number>();
  const next: unknown[] = [];
  for (const { raw, index } of combined) {
    const resolved = resolveOpenApiRef(document, raw);
    let normalized = raw;
    if (resolved?.in === "path" && typeof resolved.name === "string") {
      let name = resolved.name;
      name = renamed.get(name) ?? name;
      if (!names.includes(name)) continue;
      if (name !== resolved.name || resolved.required !== true) normalized = { ...resolved, name, required: true };
    }
    if (index >= 0) indexes.set(index, next.length);
    next.push(normalized);
  }
  const covered = new Set([...next, ...(Array.isArray(target.parameters) ? target.parameters : [])].map((raw) => parameterKey(document, raw)));
  for (const name of names) if (!covered.has(`path:${name}`)) next.push({ name, in: "path", required: true, schema: { type: "string" } });
  if (next.length) operation.parameters = next;
  else delete operation.parameters;
  return indexes;
}

export function createOpenApiOperation(document: OpenApiObject, input: OpenApiOperationAddress & { summary: string; tags: string[] }): OpenApiObject {
  const error = validateOpenApiOperationAddress(document, input);
  if (error) throw new Error(error);
  const next = clone(document);
  const paths = isRecord(next.paths) ? next.paths : {};
  next.paths = paths;
  const path = input.path.trim();
  const method = input.method.toLowerCase();
  const target = pathItem(next, path) ?? {};
  const operation: OpenApiObject = { responses: { "200": { description: "OK" } } };
  if (input.summary.trim()) operation.summary = input.summary.trim();
  const tags = [...new Set(input.tags.map((tag) => tag.trim()).filter(Boolean))];
  if (tags.length) operation.tags = tags;
  matchPathParameters(next, operation, target, path);
  target[method] = operation;
  paths[path] = target;
  return next;
}

function pointer(path: string, method: string): string {
  return `#/paths/${path.replaceAll("~", "~0").replaceAll("/", "~1")}/${method}`;
}

/** Operation moves share the literal-safe contract reference visitor. */
function rewriteReferences(value: unknown, rewrite: (reference: string) => string | undefined) {
  visitOpenApiReferences(value, (reference) => {
    if (!reference.startsWith("#/")) return;
    try {
      return rewrite(decodeURIComponent(reference));
    } catch {
      return;
    }
  });
}

export function moveOpenApiOperation(document: OpenApiObject, previousKey: string, input: OpenApiOperationAddress): OpenApiObject {
  const error = validateOpenApiOperationAddress(document, input, previousKey);
  if (error) throw new Error(error);
  const next = clone(document);
  const previous = address(previousKey);
  const path = input.path.trim();
  const method = input.method.toLowerCase();
  if (previous.path === path && previous.method === method) return next;
  const source = pathItem(next, previous.path)!;
  const operation = source[previous.method] as OpenApiObject;
  const originalParameters = Array.isArray(operation.parameters) ? [...operation.parameters] : [];
  let parameterIndexes = new Map(originalParameters.map((_raw, index) => [index, index]));
  const target = pathItem(next, path) ?? {};
  if (path !== previous.path) {
    parameterIndexes = matchPathParameters(next, operation, target, path, previous.path, Array.isArray(source.parameters) ? source.parameters : []);
    if (operation.servers === undefined && source.servers !== undefined) operation.servers = source.servers;
  }
  delete source[previous.method];
  target[method] = operation;
  (next.paths as OpenApiObject)[path] = target;
  // Keep shared path metadata and its existing references when siblings remain.
  if (path !== previous.path && !Object.keys(source).length) delete (next.paths as OpenApiObject)[previous.path];
  const previousPointer = pointer(previous.path, previous.method);
  const targetPointer = pointer(path, method);
  const references = new Set<string>();
  rewriteReferences(next, (reference) => {
    references.add(reference);
    return undefined;
  });
  const mappings = [{ previous: previousPointer, target: targetPointer }];
  originalParameters.forEach((raw, index) => {
    const prefix = `${previousPointer}/parameters/${index}`;
    const destination = parameterIndexes.get(index);
    if (destination !== undefined) mappings.push({ previous: prefix, target: `${targetPointer}/parameters/${destination}` });
    else if ([...references].some((reference) => reference === prefix || reference.startsWith(`${prefix}/`))) {
      // A removed route parameter may still supply another API's schema.
      // Retain that referenced definition without keeping an invalid path param.
      const components = isRecord(next.components) ? next.components : {};
      const parameters = isRecord(components.parameters) ? components.parameters : {};
      let suffix = 1;
      while (Object.hasOwn(parameters, `SpecXRetainedParameter${suffix}`)) suffix += 1;
      const name = `SpecXRetainedParameter${suffix}`;
      parameters[name] = raw;
      components.parameters = parameters;
      next.components = components;
      mappings.push({ previous: prefix, target: `#/components/parameters/${name}` });
    }
  });
  mappings.sort((left, right) => right.previous.length - left.previous.length);
  rewriteReferences(next, (reference) => {
    const mapping = mappings.find((item) => reference === item.previous || reference.startsWith(`${item.previous}/`));
    return mapping ? `${mapping.target}${reference.slice(mapping.previous.length)}` : undefined;
  });
  return next;
}
