import { isRecord, resolveOpenApiRef, type OpenApiObject, type OpenApiOperationEntry } from "./openapi";

export function getOperationParameters(document: OpenApiObject | null, entry: OpenApiOperationEntry): OpenApiObject[] {
  const path = isRecord(document?.paths) ? document.paths[entry.path] : undefined;
  const inherited = isRecord(path) && Array.isArray(path.parameters) ? path.parameters : [];
  const own = Array.isArray(entry.operation.parameters) ? entry.operation.parameters : [];
  const merged = new Map<string, OpenApiObject>();
  for (const item of [...inherited, ...own]) {
    const parameter = resolveOpenApiRef(document, item);
    if (parameter) merged.set(`${String(parameter.in)}:${String(parameter.name)}`, parameter);
  }
  return [...merged.values()];
}

export type OpenApiParameterLocation = "path" | "query" | "header" | "cookie";

export type SerializedOpenApiParameter = {
  pathValue: string;
  queryEntries: Array<[string, string]>;
  headerValue: string;
  cookieEntries: Array<[string, string]>;
};

export type OpenApiParameterSerializationResult = { ok: true; value: SerializedOpenApiParameter } | { ok: false; error: "unsupported_location" | "unsupported_style" | "invalid_array" | "invalid_object" | "nested_value" };

type ParameterStyle = "simple" | "label" | "matrix" | "form" | "spaceDelimited" | "pipeDelimited" | "deepObject";

export function serializeOpenApiParameter(parameter: OpenApiObject, rawValue: string, schemaOverride?: OpenApiObject | null): OpenApiParameterSerializationResult {
  const location = parameter.in;
  if (location !== "path" && location !== "query" && location !== "header" && location !== "cookie") return { ok: false, error: "unsupported_location" };
  const name = typeof parameter.name === "string" ? parameter.name : "";
  if (!name) return { ok: false, error: "unsupported_location" };

  const defaultStyle: ParameterStyle = location === "query" || location === "cookie" ? "form" : "simple";
  const style = typeof parameter.style === "string" ? parameter.style : defaultStyle;
  if (!isStyleForLocation(location, style)) return { ok: false, error: "unsupported_style" };
  const explode = typeof parameter.explode === "boolean" ? parameter.explode : style === "form";
  const schema = schemaOverride ?? (isRecord(parameter.schema) ? parameter.schema : {});
  const valueType = schemaType(schema);
  const parsed = parseValue(rawValue, valueType, location, style, explode, name);
  if (!parsed.ok) return parsed;
  const value = parsed.value;
  const allowReserved = location === "query" && parameter.allowReserved === true;

  if (location === "path") {
    const serialized = serializePath(name, value, valueType, style as "simple" | "label" | "matrix", explode);
    return serialized === null ? { ok: false, error: "nested_value" } : { ok: true, value: { pathValue: serialized, queryEntries: [], headerValue: "", cookieEntries: [] } };
  }
  if (location === "query") {
    const serialized = serializeQuery(name, value, valueType, style as "form" | "spaceDelimited" | "pipeDelimited" | "deepObject", explode, allowReserved);
    return serialized === null ? { ok: false, error: "nested_value" } : { ok: true, value: { pathValue: "", queryEntries: serialized, headerValue: "", cookieEntries: [] } };
  }
  if (location === "header") {
    const serialized = serializeHeader(name, value, valueType, explode);
    return serialized === null ? { ok: false, error: "nested_value" } : { ok: true, value: { pathValue: "", queryEntries: [], headerValue: serialized, cookieEntries: [] } };
  }
  const serialized = serializeCookie(name, value, valueType, explode);
  return serialized === null ? { ok: false, error: "nested_value" } : { ok: true, value: { pathValue: "", queryEntries: [], headerValue: "", cookieEntries: serialized } };
}

export function readOpenApiQueryParameter(parameter: OpenApiObject, searchParams: URLSearchParams | string, schemaOverride?: OpenApiObject | null, declaredNames: string[] = []): string | undefined {
  const name = typeof parameter.name === "string" ? parameter.name : "";
  if (!name || parameter.in !== "query") return undefined;
  const style = typeof parameter.style === "string" ? parameter.style : "form";
  const explode = typeof parameter.explode === "boolean" ? parameter.explode : style === "form";
  const schema = schemaOverride ?? (isRecord(parameter.schema) ? parameter.schema : {});
  const type = schemaType(schema);
  const pairs = queryPairs(searchParams);

  if (type === "array" && style === "form" && explode) {
    const values = pairs.filter((pair) => pair.name === name).map((pair) => pair.value);
    return values.length ? JSON.stringify(values) : undefined;
  }
  if (type === "object" && style === "deepObject") {
    const prefix = `${name}[`;
    const value: Record<string, unknown> = Object.create(null);
    for (const pair of pairs) {
      if (!pair.name.startsWith(prefix) || !pair.name.endsWith("]")) continue;
      const path = [...pair.name.slice(name.length).matchAll(/\[([^\]]+)\]/g)].map((match) => match[1]!);
      if (!path.length || path.length > 12 || path.some((key) => ["__proto__", "constructor", "prototype"].includes(key) || (/^\d+$/.test(key) && Number(key) > 1000))) continue;
      let target: Record<string, unknown> | unknown[] = value;
      for (let index = 0; index < path.length; index++) {
        const key = path[index]!;
        if (index === path.length - 1) {
          Reflect.set(target, key, pair.value);
          break;
        }
        let next: unknown = Object.hasOwn(target, key) ? Reflect.get(target, key) : undefined;
        if (!isRecord(next) && !Array.isArray(next)) {
          next = /^\d+$/.test(path[index + 1]!) ? [] : Object.create(null);
          Reflect.set(target, key, next);
        }
        target = next as Record<string, unknown> | unknown[];
      }
    }
    return Object.keys(value).length ? JSON.stringify(value) : undefined;
  }
  if (type === "object" && style === "form" && explode) {
    const properties = isRecord(schema.properties) ? Object.keys(schema.properties) : pairs.map((pair) => pair.name).filter((key) => !declaredNames.includes(key) && key !== name);
    const value = Object.fromEntries(
      properties.flatMap((key) => {
        const item = pairs.find((pair) => pair.name === key)?.value;
        return item === undefined ? [] : [[key, item]];
      }),
    );
    return Object.keys(value).length ? JSON.stringify(value) : undefined;
  }

  const raw = pairs.find((pair) => pair.name === name)?.rawValue;
  if (raw === undefined) return undefined;
  if (type === "primitive") return decodeQueryValue(raw);
  if (type === "array") return JSON.stringify(raw.split(querySeparator(style)).map(decodeQueryValue));
  if (style === "spaceDelimited" || style === "pipeDelimited") return undefined;
  const parts = raw.split(",").map(decodeQueryValue);
  if (parts.length % 2 !== 0) return decodeQueryValue(raw);
  const value = Object.fromEntries(Array.from({ length: parts.length / 2 }, (_, index) => [parts[index * 2]!, parts[index * 2 + 1]!]));
  return JSON.stringify(value);
}

export function readOpenApiPathParameter(pathTemplate: string, actualPath: string, parameter: OpenApiObject, schemaOverride?: OpenApiObject | null): string | undefined {
  const name = typeof parameter.name === "string" ? parameter.name : "";
  if (!name || parameter.in !== "path") return undefined;
  const templateSegments = pathTemplate.replace(/^\//u, "").split("/");
  const pathSegments = actualPath.split("/");
  const offset = pathSegments.length - templateSegments.length;
  if (offset < 0) return undefined;
  if (templateSegments.some((segment, index) => !segment.includes("{") && safeDecode(pathSegments[offset + index] ?? "") !== safeDecode(segment))) return undefined;
  const templateIndex = templateSegments.findIndex((segment) => segment.includes(`{${name}}`));
  if (templateIndex < 0) return undefined;
  const templateSegment = templateSegments[templateIndex]!;
  const rawSegment = pathSegments[offset + templateIndex];
  if (rawSegment === undefined) return undefined;
  const match = /^([^{}]*)\{[^{}]+\}([^{}]*)$/u.exec(templateSegment);
  if (!match) return undefined;
  const [, before, after] = match;
  if (!rawSegment.startsWith(before ?? "") || !rawSegment.endsWith(after ?? "")) return undefined;
  let raw = rawSegment.slice((before ?? "").length, rawSegment.length - (after ?? "").length || undefined);

  const style = typeof parameter.style === "string" ? parameter.style : "simple";
  const explode = typeof parameter.explode === "boolean" ? parameter.explode : false;
  const schema = schemaOverride ?? (isRecord(parameter.schema) ? parameter.schema : {});
  const type = schemaType(schema);
  if (type === "primitive") {
    if (style === "matrix") {
      const prefix = `;${name}=`;
      if (!raw.startsWith(prefix)) return undefined;
      raw = raw.slice(prefix.length);
    } else if (style === "label") raw = raw.replace(/^\./u, "");
    return safeDecode(raw);
  }

  if (style === "label") raw = raw.replace(/^\./u, "");
  if (type === "array") {
    let values: string[];
    if (style === "matrix" && explode) {
      const prefix = `${name}=`;
      values = raw.split(";").flatMap((part) => (part.startsWith(prefix) ? [safeDecode(part.slice(prefix.length))] : []));
    } else {
      if (style === "matrix") {
        const prefix = `;${name}=`;
        if (!raw.startsWith(prefix)) return undefined;
        raw = raw.slice(prefix.length);
      }
      values = raw.split(style === "label" && explode ? "." : ",").map(safeDecode);
    }
    return values.length ? JSON.stringify(values) : undefined;
  }

  let parts: string[];
  if (style === "matrix" && explode) {
    parts = raw.split(";").filter(Boolean);
  } else {
    if (style === "matrix") {
      const prefix = `;${name}=`;
      if (!raw.startsWith(prefix)) return undefined;
      raw = raw.slice(prefix.length);
    }
    parts = raw.split(style === "label" && explode ? "." : ",");
  }
  const object: Record<string, string> = {};
  if (explode) {
    for (const part of parts) {
      const equals = part.indexOf("=");
      if (equals <= 0) return undefined;
      object[safeDecode(part.slice(0, equals))] = safeDecode(part.slice(equals + 1));
    }
  } else {
    if (parts.length % 2 !== 0) return undefined;
    for (let index = 0; index < parts.length; index += 2) object[safeDecode(parts[index]!)] = safeDecode(parts[index + 1]!);
  }
  return JSON.stringify(object);
}

function queryPairs(source: URLSearchParams | string): Array<{ name: string; value: string; rawValue: string }> {
  if (source instanceof URLSearchParams) return [...source.entries()].map(([name, value]) => ({ name, value, rawValue: value }));
  const queryStart = source.indexOf("?");
  if (queryStart < 0) return [];
  const hashStart = source.indexOf("#", queryStart);
  const query = source.slice(queryStart + 1, hashStart < 0 ? undefined : hashStart);
  return query
    .split("&")
    .filter(Boolean)
    .map((entry) => {
      const separator = entry.indexOf("=");
      const rawName = separator < 0 ? entry : entry.slice(0, separator);
      const rawValue = separator < 0 ? "" : entry.slice(separator + 1);
      return { name: decodeQueryValue(rawName), value: decodeQueryValue(rawValue), rawValue };
    });
}

function decodeQueryValue(value: string): string {
  return safeDecode(value.replace(/\+/gu, "%20"));
}

function querySeparator(style: string): RegExp {
  if (style === "spaceDelimited") return /(?:%20|\s)+/iu;
  if (style === "pipeDelimited") return /(?:%7C|\|)/iu;
  return /,/u;
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function isStyleForLocation(location: OpenApiParameterLocation, style: string): style is ParameterStyle {
  if (location === "path") return style === "simple" || style === "label" || style === "matrix";
  if (location === "query") return style === "form" || style === "spaceDelimited" || style === "pipeDelimited" || style === "deepObject";
  if (location === "header") return style === "simple";
  return style === "form";
}

function schemaType(schema: OpenApiObject): "array" | "object" | "primitive" {
  if (schema.type === "array") return "array";
  if (schema.type === "object" || isRecord(schema.properties)) return "object";
  return "primitive";
}

function parseValue(raw: string, type: "array" | "object" | "primitive", location: OpenApiParameterLocation, style: string, explode: boolean, name: string): { ok: true; value: unknown } | { ok: false; error: "invalid_array" | "invalid_object" } {
  const value = raw.trim();
  if (type === "primitive") return { ok: true, value: raw };
  if (type === "array") {
    if (value.startsWith("[")) {
      try {
        const parsed: unknown = JSON.parse(value);
        if (Array.isArray(parsed)) return { ok: true, value: parsed };
      } catch {
        return { ok: false, error: "invalid_array" };
      }
    }
    if (!value) return { ok: true, value: [] };
    const separator = style === "spaceDelimited" ? /\s+/u : style === "pipeDelimited" ? /\|/u : /,/u;
    return { ok: true, value: value.split(separator).filter((part) => part.length > 0) };
  }
  if (value.startsWith("{")) {
    try {
      const parsed: unknown = JSON.parse(value);
      if (isRecord(parsed)) return { ok: true, value: parsed };
    } catch {
      return { ok: false, error: "invalid_object" };
    }
    return { ok: false, error: "invalid_object" };
  }
  if (!value) return { ok: true, value: {} };
  const separator = style === "label" && explode ? "." : style === "matrix" && explode ? ";" : style === "form" && explode ? (location === "query" ? "&" : ";") : ",";
  const parts = value.split(separator);
  const result: Record<string, unknown> = {};
  if (explode) {
    for (const part of parts) {
      const equals = part.indexOf("=");
      if (equals <= 0) return { ok: false, error: "invalid_object" };
      result[part.slice(0, equals)] = part.slice(equals + 1);
    }
  } else {
    if (parts.length % 2 !== 0) return { ok: false, error: "invalid_object" };
    for (let index = 0; index < parts.length; index += 2) result[parts[index]!] = parts[index + 1]!;
  }
  if (style === "deepObject" && Object.keys(result).some((key) => key.startsWith(`${name}[`))) {
    return {
      ok: true,
      value: Object.fromEntries(Object.entries(result).map(([key, item]) => [key.slice(name.length + 1, -1), item])),
    };
  }
  return { ok: true, value: result };
}

function encode(value: unknown, allowReserved = false): string | null {
  if (isRecord(value) || Array.isArray(value)) return null;
  const serialized = value === null ? "null" : String(value);
  let output = encodeURIComponent(serialized)
    .replace(/%7B%7B/gi, "{{")
    .replace(/%7D%7D/gi, "}}")
    .replace(/%3C%3C/gi, "<<")
    .replace(/%3E%3E/gi, ">>");
  if (allowReserved) {
    output = output.replace(/%(3A|2F|3F|5B|5D|40|21|24|27|28|29|2A|2C|3B)/gi, (encoded) => decodeURIComponent(encoded));
  }
  return output;
}

function entries(value: unknown): Array<[string, unknown]> | null {
  if (Array.isArray(value)) return value.map((item, index) => [String(index), item]);
  if (isRecord(value)) return Object.entries(value);
  return null;
}

function encodeValues(value: unknown): string[] | null {
  const values = Array.isArray(value) ? value : [value];
  const output = values.map((item) => encode(item));
  return output.some((item) => item === null) ? null : (output as string[]);
}

function serializePath(name: string, value: unknown, type: "array" | "object" | "primitive", style: "simple" | "label" | "matrix", explode: boolean): string | null {
  const encodedName = encodeURIComponent(name);
  if (type === "primitive") {
    const encodedValue = encode(value);
    if (encodedValue === null) return null;
    if (style === "label") return `.${encodedValue}`;
    if (style === "matrix") return `;${encodedName}=${encodedValue}`;
    return encodedValue;
  }
  if (type === "array") {
    const items = encodeValues(value);
    if (!items) return null;
    if (style === "label") return `.${items.join(explode ? "." : ",")}`;
    if (style === "matrix") return explode ? items.map((item) => `;${encodedName}=${item}`).join("") : `;${encodedName}=${items.join(",")}`;
    return items.join(",");
  }
  const pairs = entries(value);
  if (!pairs) return null;
  const encodedPairs = pairs.map(([key, item]) => {
    const encodedKey = encodeURIComponent(key);
    const encodedValue = encode(item);
    return encodedValue === null ? null : ([encodedKey, encodedValue] as const);
  });
  if (encodedPairs.some((pair) => pair === null)) return null;
  const safePairs = encodedPairs as Array<readonly [string, string]>;
  const flat = safePairs.flatMap(([key, item]) => [key, item]);
  if (style === "label") return `.${explode ? safePairs.map(([key, item]) => `${key}=${item}`).join(".") : flat.join(",")}`;
  if (style === "matrix") return explode ? safePairs.map(([key, item]) => `;${key}=${item}`).join("") : `;${encodedName}=${flat.join(",")}`;
  return explode ? safePairs.map(([key, item]) => `${key}=${item}`).join(",") : flat.join(",");
}

function serializeQuery(name: string, value: unknown, type: "array" | "object" | "primitive", style: "form" | "spaceDelimited" | "pipeDelimited" | "deepObject", explode: boolean, allowReserved: boolean): Array<[string, string]> | null {
  const encodedName = encodeURIComponent(name);
  if (type === "primitive") {
    const encodedValue = encode(value, allowReserved);
    return encodedValue === null ? null : [[encodedName, encodedValue]];
  }
  if (type === "array") {
    const items = (Array.isArray(value) ? value : []).map((item) => encode(item, allowReserved));
    if (items.some((item) => item === null)) return null;
    const safeItems = items as string[];
    if (style === "form" && explode) return safeItems.map((item) => [encodedName, item]);
    const separator = style === "spaceDelimited" ? "%20" : style === "pipeDelimited" ? "|" : ",";
    return safeItems.length ? [[encodedName, safeItems.join(separator)]] : [];
  }
  if (style === "spaceDelimited" || style === "pipeDelimited") return null;
  if (style === "deepObject") {
    const result: Array<[string, string]> = [];
    const visit = (item: unknown, key: string, depth: number): boolean => {
      if (depth > 12 || result.length > 2000) return false;
      if (isRecord(item) || Array.isArray(item)) return Object.entries(item).every(([child, value]) => visit(value, `${key}[${child}]`, depth + 1));
      const encoded = encode(item, allowReserved);
      if (encoded === null) return false;
      result.push([encodeURIComponent(key), encoded]);
      return true;
    };
    return visit(value, name, 0) ? result : null;
  }
  const pairs = entries(value);
  if (!pairs) return null;
  const encodedPairs = pairs.map(([key, item]) => {
    const encodedKey = encodeURIComponent(key);
    const encodedValue = encode(item, allowReserved);
    return encodedValue === null ? null : ([encodedKey, encodedValue] as const);
  });
  if (encodedPairs.some((pair) => pair === null)) return null;
  const safePairs = encodedPairs as Array<readonly [string, string]>;
  if (explode) return safePairs.map(([key, item]) => [key, item]);
  const flat = safePairs.flatMap(([key, item]) => [key, item]);
  return [[encodedName, flat.join(",")]];
}

function serializeHeader(_name: string, value: unknown, type: "array" | "object" | "primitive", explode: boolean): string | null {
  if (type === "primitive") return String(value);
  if (type === "array") {
    if (!Array.isArray(value) || value.some((item) => isRecord(item) || Array.isArray(item))) return null;
    return value.map((item) => String(item)).join(",");
  }
  const pairs = entries(value);
  if (!pairs) return null;
  const encodedPairs = pairs.map(([key, item]) => {
    return isRecord(item) || Array.isArray(item) ? null : `${key}=${String(item)}`;
  });
  if (encodedPairs.some((item) => item === null)) return null;
  if (explode) return (encodedPairs as string[]).join(",");
  return pairs.flatMap(([key, item]) => [key, encode(item) ?? ""]).join(",");
}

function serializeCookie(name: string, value: unknown, type: "array" | "object" | "primitive", explode: boolean): Array<[string, string]> | null {
  if (type === "primitive") {
    const item = encode(value);
    return item === null ? null : [[name, item]];
  }
  if (type === "array") {
    const items = encodeValues(value);
    if (!items) return null;
    return explode ? items.map((item) => [name, item]) : [[name, items.join(",")]];
  }
  const pairs = entries(value);
  if (!pairs) return null;
  const encodedPairs = pairs.map(([key, item]) => {
    const encodedValue = encode(item);
    return encodedValue === null ? null : ([key, encodedValue] as const);
  });
  if (encodedPairs.some((pair) => pair === null)) return null;
  const safePairs = encodedPairs as Array<readonly [string, string]>;
  if (explode) return safePairs.map(([key, item]) => [key, item]);
  return [[name, safePairs.flatMap(([key, item]) => [key, item]).join(",")]];
}
