/** Apifox dynamic-value expressions: Faker v9 + literal-only arguments, no eval. */
import { APIFOX_DYNAMIC_METHODS } from "./openapiDynamicCatalog";
import { Faker, en, zh_CN, base } from "@faker-js/faker";
import JSON5 from "json5";
import { md5, sha1 } from "@noble/hashes/legacy";
import { sha256, sha512 } from "@noble/hashes/sha2";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";
import { addDays, addWeeks, addMonths, addQuarters, addYears, addHours, addMinutes, addSeconds, format, formatISO, formatISO9075, formatRFC3339, formatRFC7231 } from "date-fns";

export type DynamicLocale = "zh_CN" | "en";
export class OpenApiVariableError extends Error {
  constructor(
    public readonly expression: string,
    public readonly reason: "invalid" | "unknown" | "cycle" | "limit" = "invalid",
  ) {
    super(`dynamic_variable_${reason}: ${expression}`);
  }
}
const generators = { en: new Faker({ locale: [en, base] }), zh_CN: new Faker({ locale: [zh_CN, en, base] }) };
const custom = [
  "timestamp",
  "guid",
  "randomUUID",
  "randomInt",
  "isoTimestamp",
  "date.now",
  "date.timestamp",
  "date.millisecondsTimestamp",
  "date.isoTimestamp",
  "person.middleName",
  "image.avatarLegacy",
  "location.region",
  "phone.mobile",
  "phone.landline",
  "person.idCard",
  "internet.qq",
  "person.sexType",
  "helpers.rangeToNumber",
  "airline.airlineName",
  "airline.airlineIataCode",
  "airline.airportName",
  "airline.airportIataCode",
  "airline.airplaneName",
  "airline.airplaneIataTypeCode",
  "finance.currencyCode",
  "finance.currencyName",
  "finance.currencySymbol",
  "science.chemicalElementName",
  "science.chemicalElementSymbol",
  "science.chemicalElementAtomicNumber",
  "science.unitName",
  "science.unitSymbol",
  "location.nearbyGPSCoordinateLatitude",
  "location.nearbyGPSCoordinateLongitude",
];
const blocked = new Set(["constructor", "prototype", "__proto__", "faker", "seed", "setDefaultRefDate"]);
const registry = new Set<string>([...custom, ...APIFOX_DYNAMIC_METHODS]);
export const OPENAPI_DYNAMIC_VARIABLES = [...registry].sort().map((name) => ({ name, group: name.includes(".") ? name.split(".")[0]! : "common", expression: `{{$${name}}}` }));
const MAX_OUTPUT = 1024 * 1024;
const record = (value: unknown): value is Record<string, unknown> => Boolean(value && typeof value === "object" && !Array.isArray(value));

/** Split delimiters only outside quoted literals and containers. */
function split(source: string, delimiter: string): string[] {
  let quote = "",
    escaped = false,
    depth = 0,
    start = 0;
  const result: string[] = [];
  for (let i = 0; i < source.length; i++) {
    const c = source[i]!;
    if (quote) {
      if (escaped) escaped = false;
      else if (c === "\\") escaped = true;
      else if (c === quote) quote = "";
      continue;
    }
    if (c === "'" || c === '"') quote = c;
    else if ("([{".includes(c)) depth++;
    else if (")]}".includes(c)) {
      depth--;
      if (depth < 0) throw new Error("unbalanced");
    } else if (c === delimiter && depth === 0) {
      result.push(source.slice(start, i).trim());
      start = i + 1;
    }
  }
  if (quote || depth) throw new Error("unbalanced");
  result.push(source.slice(start).trim());
  return result;
}
type Call = { name: string; args: unknown[]; options: Record<string, unknown> };
function call(source: string): Call {
  const match = /^([A-Za-z][\w.]*)\s*(?:\(([\s\S]*)\))?$/.exec(source.trim());
  if (!match || match[1]!.split(".").some((key) => blocked.has(key))) throw new Error("invalid_call");
  const args: unknown[] = [],
    options: Record<string, unknown> = {};
  for (const part of match[2]?.trim() ? split(match[2], ",") : []) {
    const named = /^([A-Za-z]\w*)\s*=([\s\S]+)$/.exec(part);
    let value: unknown = JSON5.parse(named ? named[2]! : part);
    if (typeof value === "string" && value.startsWith("[") && value.endsWith("]")) {
      try {
        value = JSON5.parse(value);
      } catch {
        /* Literal brackets remain text. */
      }
    }
    validateLiteral(value);
    if (named) {
      if (blocked.has(named[1]!)) throw new Error("invalid_key");
      options[named[1]!] = value;
    } else args.push(value);
  }
  return { name: match[1]!, args, options };
}
function validateLiteral(value: unknown, depth = 0) {
  if (depth > 12) throw new Error("literal_depth");
  if (typeof value === "number" && !Number.isFinite(value)) throw new Error("invalid_number");
  if (typeof value === "string" && value.length > 8192) throw new Error("literal_size");
  if (Array.isArray(value)) {
    if (value.length > 1000) throw new Error("literal_size");
    value.forEach((item) => validateLiteral(item, depth + 1));
  }
  if (record(value))
    for (const [key, item] of Object.entries(value)) {
      if (blocked.has(key)) throw new Error("invalid_key");
      validateLiteral(item, depth + 1);
    }
}
function bounded(options: Record<string, unknown>, keys: string[]) {
  for (const key of keys) if (typeof options[key] === "number" && (options[key] < 0 || options[key] > 1000)) throw new Error("generation_limit");
}
function valueText(value: unknown): string {
  const result = value instanceof Date ? format(value, "yyyy-MM-dd HH:mm:ss") : typeof value === "object" ? JSON.stringify(value, (_, item) => (typeof item === "bigint" ? String(item) : item)) : String(value ?? "");
  if (result.length > MAX_OUTPUT) throw new Error("output_limit");
  return result;
}
function generate({ name, args, options: original }: Call, locale: DynamicLocale, now: Date): unknown {
  const options = { ...original };
  const checkCount = (value: unknown): void => {
    if (typeof value === "number" && (!Number.isInteger(value) || value < 0 || value > 1000)) throw new Error("generation_limit");
    if (record(value)) {
      checkCount(value.min);
      checkCount(value.max);
    }
  };
  if (name.startsWith("string.")) {
    checkCount(args[0]);
    if (name === "string.fromCharacters") checkCount(args[1]);
    if (name === "date.betweens" && record(args[0])) checkCount(args[0].count);
    checkCount(options.length);
    checkCount(options.min);
    checkCount(options.max);
  }
  if (name.startsWith("lorem.") || name === "word.words") {
    checkCount(args[0]);
    checkCount(options.count);
    checkCount(options.min);
    checkCount(options.max);
  }
  if (name === "date.betweens") {
    checkCount(options.count);
    checkCount(options.min);
    checkCount(options.max);
  }

  checkCount(options.length);
  checkCount(options.count);
  const selectedLocale = options.locale;
  if (selectedLocale !== undefined && selectedLocale !== "en" && selectedLocale !== "zh_CN" && selectedLocale !== "zh-CN") throw new Error("unsupported_locale");
  const f = generators[selectedLocale === "en" ? "en" : selectedLocale ? "zh_CN" : locale];
  delete options.locale;
  f.setDefaultRefDate(now);
  if (["timestamp", "date.timestamp"].includes(name)) return Math.floor(now.getTime() / 1000);
  if (name === "date.millisecondsTimestamp") return now.getTime();
  if (["isoTimestamp", "date.isoTimestamp"].includes(name)) return now.toISOString();
  if (name === "date.now") return now;
  if (["guid", "randomUUID"].includes(name)) return f.string.uuid();
  if (name === "randomInt") return f.number.int({ min: 0, max: 1000, ...options });
  if (name === "helper.autoIncrementEnvVar") return Number(options.start ?? 0) + Number(options.step ?? 1);
  if (name === "location.region") return f.location.state();
  if (name === "phone.mobile") return `1${f.helpers.arrayElement(["3", "5", "7", "8", "9"])}${f.string.numeric(9)}`;
  if (name === "phone.landline") return `0${f.string.numeric(2)}-${f.string.numeric(8)}`;
  if (name === "internet.qq") return f.string.numeric({ length: 10, allowLeadingZeros: false });
  if (name === "person.prefix") return generators.en.person.prefix(options.sex as "male" | "female" | undefined);
  if (name === "person.sexType") return f.person.sexType();
  if (name === "person.idCard") {
    const first = `110101${format(f.date.birthdate({ mode: "age", min: 18, max: 65 }), "yyyyMMdd")}${f.string.numeric(3)}`;
    const weights = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    return first + "10X98765432"[[...first].reduce((sum, digit, i) => sum + Number(digit) * weights[i]!, 0) % 11];
  }
  if (name === "helpers.rangeToNumber") return f.number.int({ min: 1, max: 10, ...options });
  const projections: Record<string, [() => unknown, string]> = {
    "airline.airlineName": [() => f.airline.airline(), "name"],
    "airline.airlineIataCode": [() => f.airline.airline(), "iataCode"],
    "airline.airportName": [() => f.airline.airport(), "name"],
    "airline.airportIataCode": [() => f.airline.airport(), "iataCode"],
    "airline.airplaneName": [() => f.airline.airplane(), "name"],
    "airline.airplaneIataTypeCode": [() => f.airline.airplane(), "iataTypeCode"],
    "finance.currencyCode": [() => f.finance.currency(), "code"],
    "finance.currencyName": [() => f.finance.currency(), "name"],
    "finance.currencySymbol": [() => f.finance.currency(), "symbol"],
    "science.chemicalElementName": [() => f.science.chemicalElement(), "name"],
    "science.chemicalElementSymbol": [() => f.science.chemicalElement(), "symbol"],
    "science.chemicalElementAtomicNumber": [() => f.science.chemicalElement(), "atomicNumber"],
    "science.unitName": [() => f.science.unit(), "name"],
    "science.unitSymbol": [() => f.science.unit(), "symbol"],
  };
  const projection = projections[name];
  if (projection) {
    const value = projection[0]();
    return record(value) ? value[projection[1]] : value;
  }
  if (name.startsWith("location.nearbyGPSCoordinate")) {
    const pair = f.location.nearbyGPSCoordinate();
    return pair[name.endsWith("Latitude") ? 0 : 1];
  }
  if (!registry.has(name)) throw new OpenApiVariableError(name, "unknown");
  if (name === "date.between" || name === "date.betweens") {
    options.from ??= new Date(now.getFullYear() - 1, now.getMonth(), now.getDate()).toISOString();
    options.to ??= now.toISOString();
  }
  if (name.startsWith("string.")) {
    bounded(options, ["length", "min", "max"]);
    if (options.min !== undefined || options.max !== undefined) {
      options.length = { min: options.min ?? options.max, max: options.max ?? options.min };
      delete options.min;
      delete options.max;
    }
  }
  if (name === "date.betweens") {
    bounded(options, ["min", "max", "count"]);
    if (options.min !== undefined || options.max !== undefined) {
      options.count = { min: options.min ?? 1, max: options.max ?? options.min ?? 3 };
      delete options.min;
      delete options.max;
    }
  }
  if (name === "lorem.paragraphs" || name === "lorem.sentences" || name === "lorem.words" || name === "word.words" || name === "lorem.lines") {
    bounded(options, ["min", "max", "count"]);
    if (!args.length) args = [options.count ?? (options.min !== undefined || options.max !== undefined ? { min: options.min ?? 1, max: options.max ?? options.min } : 3)];
  }
  if (["string.nanoid", "string.sample", "string.symbol"].includes(name)) {
    const length = args[0] ?? options.length;
    if (typeof length === "number" && (length < 0 || length > 1000)) throw new Error("generation_limit");
    return Reflect.apply(Reflect.get(f.string, name.split(".")[1]!), f.string, [typeof length === "object" ? f.number.int(length as { min: number; max: number }) : Number(length ?? (name === "string.nanoid" ? 21 : 1))]);
  }
  if (name === "helpers.fromRegExp") {
    const pattern = String(args[0] ?? options.pattern ?? "[A-Z0-9]{4}");
    let budget = 1;
    for (const repeat of pattern.matchAll(/\{(\d+)(?:,(\d*))?\}/g)) {
      const count = Number(repeat[2] ?? repeat[1]);
      budget *= Math.max(1, count);
      if (count > 1000 || budget > 8192) throw new Error("regexp_limit");
    }
    if (pattern.length > 256) throw new Error("regexp_limit");
    return f.helpers.fromRegExp(pattern);
  }
  const positional: Record<string, string[]> = {
    "helpers.arrayElement": ["array"],
    "helpers.arrayElements": ["array", "count"],
    "helpers.replaceSymbols": ["string"],
    "helpers.slugify": ["string"],
    "helpers.replaceCreditCardSymbols": ["string", "symbol"],
    "string.fromCharacters": ["characters", "length"],
    "image.dataUri": ["width", "height", "color"],
  };
  if (positional[name] && !args.length) args = positional[name]!.map((key) => options[key]);
  if (name === "helpers.arrayElement" || name === "helpers.arrayElements") {
    if (!args[0]) args[0] = ["a", "b", "c"];
  }
  if (name === "string.fromCharacters" && !args[0]) args[0] = "abc123";
  if (name === "helpers.replaceSymbols" && !args[0]) args[0] = "##??**";
  if (name === "helpers.slugify" && !args[0]) args[0] = "example text";
  const [group, method] = name.split(".");
  const module = Reflect.get(f, group!);
  const fn = Reflect.get(module, method!) as (...values: unknown[]) => unknown;
  return Reflect.apply(fn, module, args.length ? args : Object.keys(options).length ? [options] : []);
}
const dateTransforms: Record<string, (date: Date, amount: number) => Date> = { addDays, addWeeks, addMonths, addQuarters, addYears, addHours, addMinutes, addSeconds };
function transform(value: unknown, pipe: Call): unknown {
  const text = valueText(value);
  if (dateTransforms[pipe.name]) return dateTransforms[pipe.name]!(value instanceof Date ? value : new Date(text), Number(pipe.args[0] ?? 0));
  if (pipe.name === "format") return format(value instanceof Date ? value : new Date(text), String(pipe.args[0] ?? "yyyy-MM-dd HH:mm:ss"));
  const dates: Record<string, (date: Date) => string> = { formatISO, formatISO9075, formatRFC3339, formatRFC7231 };
  if (dates[pipe.name]) return dates[pipe.name]!(value instanceof Date ? value : new Date(text));
  if (["upper", "upperCase"].includes(pipe.name)) return text.toUpperCase();
  if (["lower", "lowerCase"].includes(pipe.name)) return text.toLowerCase();
  if (pipe.name === "trim") return text.trim();
  if (pipe.name === "urlencode" || pipe.name === "encodeURIComponent") return encodeURIComponent(text);
  if (pipe.name === "base64") return btoa(String.fromCharCode(...new TextEncoder().encode(text)));
  const hashes: Record<string, (input: Uint8Array) => Uint8Array> = { md5, sha1, sha256, sha512 };
  if (hashes[pipe.name]) return bytesToHex(hashes[pipe.name]!(utf8ToBytes(text)));
  if (pipe.name === "json") return JSON.stringify(value);
  throw new OpenApiVariableError(pipe.name, "unknown");
}
export function evaluateDynamicExpression(expression: string, options: { locale?: DynamicLocale; now?: Date } = {}): unknown {
  const raw = expression
    .replace(/^\{\{\s*/, "")
    .replace(/\s*\}\}$/, "")
    .trim();
  try {
    if (raw.length > 8192) throw new Error("expression_limit");
    const normalized = raw.replace(/\)\.(format\()/g, ")|$1");
    const [first, ...pipes] = split(normalized, "|");
    let value: unknown = first!.startsWith("$") ? generate(call(first!.slice(1)), options.locale ?? "zh_CN", options.now ?? new Date()) : JSON5.parse(first!);
    for (const pipe of pipes) value = transform(value, call(pipe));
    valueText(value);
    return value;
  } catch (error) {
    if (error instanceof OpenApiVariableError) throw error;
    throw new OpenApiVariableError(raw);
  }
}

/** One resolver per send. Drafts are never expanded or changed during rendering. */
export function createOpenApiVariableResolver(options: { locale?: DynamicLocale; now?: Date; onVariableUpdate?: (name: string, value: string) => void } = {}) {
  let generated = 0;
  function resolve(input: string, variables: Record<string, string>, stack: string[] = []): string {
    if (stack.length > 16 || input.length > MAX_OUTPUT) throw new OpenApiVariableError(stack.at(-1) ?? "", "limit");
    const replace = (match: string, expression: string): string => {
      const key = expression.trim();
      if (Object.hasOwn(variables, key)) {
        if (stack.includes(key)) throw new OpenApiVariableError(key, "cycle");
        return resolve(variables[key]!, variables, [...stack, key]);
      }
      if (!key.startsWith("$") && !/^['"]/.test(key)) return match;
      if (++generated > 2000) throw new OpenApiVariableError(key, "limit");
      if (key.startsWith("$helper.autoIncrementEnvVar")) {
        const method = call(key.slice(1));
        const name = String(method.args[0] ?? method.options.name ?? method.options.variableName ?? "counter");
        if (blocked.has(name) || name.length > 256) throw new OpenApiVariableError(key);
        const number = Number(variables[name] ?? method.options.start ?? 0) + Number(method.options.step ?? 1);
        if (!Number.isFinite(number)) throw new OpenApiVariableError(key);
        variables[name] = String(number);
        options.onVariableUpdate?.(name, String(number));
        return String(number);
      }
      return valueText(evaluateDynamicExpression(key, options));
    };
    let result = "";
    for (let index = 0; index < input.length;) {
      if (input.startsWith("{{", index)) {
        const end = expressionEnd(input, index);
        result += replace(input.slice(index, end), input.slice(index + 2, end - 2));
        index = end;
      } else if (input.startsWith("<<", index)) {
        const end = input.indexOf(">>", index + 2);
        if (end < 0) {
          result += input.slice(index);
          break;
        }
        result += replace(input.slice(index, end + 2), input.slice(index + 2, end));
        index = end + 2;
      } else result += input[index++];
      if (result.length > MAX_OUTPUT) throw new OpenApiVariableError("", "limit");
    }
    if (result.length > MAX_OUTPUT) throw new OpenApiVariableError("", "limit");
    return result;
  }
  return {
    expand: (input: string, variables: Record<string, string> = {}) => resolve(input, variables),
    expandJson: (input: string, variables: Record<string, string> = {}) => {
      // Expressions are atomic even when their literal arguments contain quotes.
      // Only generated text inside JSON strings is escaped; numeric/boolean
      // placeholders outside strings retain their JSON types.
      let result = "",
        inString = false;
      for (let index = 0; index < input.length;) {
        if (input.startsWith("{{", index)) {
          const end = expressionEnd(input, index, inString);
          const value = resolve(inString ? decodeJsonFragment(input.slice(index, end)) : input.slice(index, end), variables);
          result += inString ? JSON.stringify(value).slice(1, -1) : value;
          index = end;
        } else if (input[index] === "\\" && inString) {
          result += input.slice(index, index + 2);
          index += 2;
        } else {
          const char = input[index++]!;
          if (char === '"') inString = !inString;
          result += char;
        }
      }
      if (result.length > MAX_OUTPUT) throw new OpenApiVariableError("", "limit");
      return result;
    },
  };
}

function decodeJsonFragment(value: string): string {
  return value.replace(/\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4})/g, (escape) => JSON.parse('"' + escape + '"') as string);
}

export function dynamicExpressionRangeAt(input: string, cursor: number): { from: number; to: number } | null {
  if (cursor < 2) return null;
  const from = input.lastIndexOf("{{", cursor - 2);
  if (from === -1 || !/^\{\{\s*\$/.test(input.slice(from, cursor))) return null;
  try {
    const to = expressionEnd(input, from);
    return to >= cursor ? { from, to } : null;
  } catch {
    return null;
  }
}

function expressionEnd(input: string, start: number, jsonString = false): number {
  let quote = "",
    escaped = false,
    depth = 0;
  for (let index = start + 2; index < input.length; index++) {
    let char = input[index]!;
    if (jsonString && char === "\\") {
      const escape = /^\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4})/.exec(input.slice(index));
      if (escape) {
        char = decodeJsonFragment(escape[0]);
        index += escape[0].length - 1;
      }
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === quote) quote = "";
      continue;
    }
    if (char === '"' || char === "'") quote = char;
    else if (input.startsWith("}}", index) && depth === 0) return index + 2;
    else if ("([{".includes(char)) depth++;
    else if (")]}".includes(char)) depth--;
  }
  throw new OpenApiVariableError(input.slice(start));
}
