import type { WorkbenchRequest, WorkbenchRow } from "../openapiWorkbench";

export type CurlFileHint = { name?: string; path: string; contentType?: string; fileName?: string };
export type CurlImportError = { code: "syntax" | "shell" | "option" | "value" | "url" | "multiple" | "conflict"; option?: string };
export type CurlImportResult = { request: WorkbenchRequest; formFiles: CurlFileHint[]; bodyFile?: CurlFileHint; warnings: Array<"files" | "network"> } | { error: CurlImportError };

/** Parse POSIX shell words without evaluating a shell or expanding variables. */
function shellWords(source: string): string[] | CurlImportError {
  const words: string[] = [];
  let word = "";
  let active = false;
  let quote = "";
  for (let i = 0; i < source.length; i += 1) {
    const c = source[i]!;
    if (quote === "'") {
      if (c === "'") quote = "";
      else word += c;
      continue;
    }
    if (quote === "ansi") {
      if (c === "'") quote = "";
      else if (c === "\\") {
        const next = source[++i];
        const escapes: Record<string, string> = { n: "\n", r: "\r", t: "\t", b: "\b", f: "\f", v: "\v", "\\": "\\", "'": "'", '"': '"' };
        if (next === undefined) return { code: "syntax" };
        if (Object.hasOwn(escapes, next)) word += escapes[next];
        else if (["x", "u", "U"].includes(next)) {
          const count = next === "x" ? 2 : next === "u" ? 4 : 8;
          const digits = source.slice(i + 1, i + 1 + count);
          if (digits.length !== count || !/^[0-9a-f]+$/i.test(digits)) return { code: "syntax" };
          const point = Number.parseInt(digits, 16);
          if (point > 0x10ffff || point === 0 || (point >= 0xd800 && point <= 0xdfff)) return { code: "syntax" };
          word += String.fromCodePoint(point);
          i += count;
        } else return { code: "syntax" };
      } else word += c;
      continue;
    }
    if (c === "\\") {
      const next = source[i + 1];
      if (next === undefined) return { code: "syntax" };
      if (next === "\n" || (next === "\r" && source[i + 2] === "\n")) {
        i += next === "\r" ? 2 : 1;
        continue;
      }
      if (quote === '"' && !["$", "`", '"', "\\"].includes(next)) word += "\\";
      else {
        word += next;
        i += 1;
      }
      active = true;
    } else if (c === "`" || (c === "$" && source[i + 1] !== "'")) return { code: "shell" };
    else if (quote === '"') {
      if (c === '"') quote = "";
      else word += c;
    } else if (c === "$" && source[i + 1] === "'") {
      quote = "ansi";
      active = true;
      i += 1;
    } else if (c === "'" || c === '"') {
      quote = c;
      active = true;
    } else if (/\s/.test(c)) {
      if (active) words.push(word);
      word = "";
      active = false;
    } else if (c === "#" && !active) {
      while (i < source.length && source[i] !== "\n") i += 1;
    } else if (/[;&|<>]/.test(c)) return { code: "shell" };
    else {
      word += c;
      active = true;
    }
  }
  if (quote) return { code: "syntax" };
  if (active) words.push(word);
  return words;
}

const valueOptions: Record<string, string> = {
  X: "request",
  H: "header",
  d: "data",
  b: "cookie",
  u: "user",
  A: "user-agent",
  e: "referer",
  F: "form",
  url: "url",
  request: "request",
  header: "header",
  data: "data",
  "data-ascii": "data",
  "data-raw": "data-raw",
  "data-binary": "data-binary",
  "data-urlencode": "data-urlencode",
  json: "json",
  cookie: "cookie",
  user: "user",
  "user-agent": "user-agent",
  referer: "referer",
  form: "form",
  "form-string": "form-string",
  "oauth2-bearer": "bearer",
  "url-query": "url-query",
  m: "timeout",
  "max-time": "timeout",
  "connect-timeout": "timeout",
};
const switchOptions: Record<string, string> = {
  G: "get",
  I: "head",
  L: "network",
  s: "network",
  S: "network",
  v: "network",
  i: "network",
  f: "network",
  g: "network",
  get: "get",
  head: "head",
  location: "network",
  compressed: "network",
  silent: "network",
  "show-error": "network",
  verbose: "network",
  include: "network",
  fail: "network",
  "fail-with-body": "network",
  globoff: "network",
  "http1.1": "network",
  http2: "network",
  basic: "basic",
};
function header(name: string, value: string): WorkbenchRow {
  return { name, value, enabled: true };
}
function formEncode(value: string): string {
  return encodeURIComponent(value).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
}
function encodedData(value: string): string | null {
  const equals = value.indexOf("=");
  const at = value.indexOf("@");
  if (equals < 0 && at >= 0) return null;
  return equals < 0 ? formEncode(value) : `${value.slice(0, equals)}=${formEncode(value.slice(equals + 1))}`;
}
function utf8Base64(value: string): string {
  return btoa(Array.from(new TextEncoder().encode(value), (byte) => String.fromCharCode(byte)).join(""));
}
function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
function safeUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return ["http:", "https:"].includes(url.protocol);
  } catch {
    return false;
  }
}

// cURL has its own quoting rules inside --form, separate from shell quoting.
function formFileParts(value: string): string[] | null {
  const parts: string[] = [];
  let part = "";
  let quoted = false;
  for (let index = 0; index < value.length; index += 1) {
    const char = value[index]!;
    if (char === '"') quoted = !quoted;
    else if (quoted && char === "\\" && ['"', "\\"].includes(value[index + 1] ?? "")) part += value[++index];
    else if (!quoted && char === ";") {
      parts.push(part);
      part = "";
    } else if (!quoted && char === ",") return null;
    else part += char;
  }
  if (quoted) return null;
  parts.push(part);
  return parts;
}

/** A single HTTP request, with actionable errors for options whose semantics cannot be reproduced. */
export function parseCurlImport(source: string): CurlImportResult {
  const words = shellWords(source.trim());
  if (!Array.isArray(words)) return { error: words };
  if (!words.length || !/^curl(?:\.exe)?$/i.test(words[0]!)) return { error: { code: "syntax" } };
  const request: WorkbenchRequest = { method: "GET", url: "", headers: [], cookies: [], body: "", mediaType: "" };
  const formFiles: CurlFileHint[] = [];
  const warnings = new Set<"files" | "network">();
  const data: string[] = [];
  const forms: Array<[string, string]> = [];
  const query: string[] = [];
  let bodyFile: CurlFileHint | undefined;
  let explicitMethod = "";
  let get = false;
  let head = false;
  let hasData = false;
  let hasForm = false;
  let json = false;
  let auth = "";
  let endOptions = false;
  for (let i = 1; i < words.length; i += 1) {
    let raw = words[i]!;
    if (raw === "--") {
      endOptions = true;
      continue;
    }
    if (!raw.startsWith("-") || endOptions) {
      if (!safeUrl(raw)) return { error: { code: "url" } };
      if (request.url) return { error: { code: "multiple" } };
      request.url = raw;
      continue;
    }
    let name = "";
    let value: string | undefined;
    if (raw.startsWith("--")) {
      const equals = raw.indexOf("=");
      name = raw.slice(2, equals < 0 ? undefined : equals);
      if (equals >= 0) value = raw.slice(equals + 1);
    } else {
      name = raw[1]!;
      if (valueOptions[name]) {
        if (raw.length > 2) value = raw.slice(2);
      } else if (raw.length > 2) {
        words.splice(i + 1, 0, `-${raw.slice(2)}`);
      }
    }
    const option = valueOptions[name] ?? switchOptions[name];
    if (!option) return { error: { code: "option", option: raw } };
    if (valueOptions[name]) {
      value ??= words[++i];
      if (value === undefined) return { error: { code: "value", option: raw } };
    } else if (value !== undefined) return { error: { code: "value", option: raw } };
    const v = value ?? "";
    if (option === "request") {
      if (!/^[!#$%&'*+.^_\x60|~0-9a-z-]+$/i.test(v)) return { error: { code: "value", option: raw } };
      explicitMethod = v.toUpperCase();
    } else if (option === "url") {
      if (!safeUrl(v)) return { error: { code: "url" } };
      if (request.url) return { error: { code: "multiple" } };
      request.url = v;
    } else if (option === "header") {
      if (v.startsWith("@")) return { error: { code: "option", option: raw } };
      const separator = v.indexOf(":");
      if (separator < 1 || /[\r\n]/.test(v)) return { error: { code: "value", option: raw } };
      const name = v.slice(0, separator).trim();
      if (!/^[!#$%&'*+.^_\x60|~0-9a-z-]+$/i.test(name)) return { error: { code: "value", option: raw } };
      request.headers.push(header(name, v.slice(separator + 1).trim()));
    } else if (option === "cookie") {
      if (!v.includes("=")) return { error: { code: "option", option: raw } };
      for (const pair of v.split(";")) {
        const eq = pair.indexOf("=");
        if (eq > 0) request.cookies.push(header(pair.slice(0, eq).trim(), pair.slice(eq + 1).trim()));
      }
    } else if (option === "user") auth = `Basic ${utf8Base64(v.includes(":") ? v : `${v}:`)}`;
    else if (option === "bearer") auth = `Bearer ${v}`;
    else if (option === "user-agent" || option === "referer") request.headers.push(header(option === "referer" ? "Referer" : "User-Agent", v));
    else if (option === "get") get = true;
    else if (option === "head") head = true;
    else if (option === "timeout") {
      if (!/^\d+(?:\.\d+)?$/.test(v) || !Number.isFinite(Number(v))) return { error: { code: "value", option: raw } };
      warnings.add("network");
    } else if (option === "network") warnings.add("network");
    else if (["data", "data-raw", "data-binary", "data-urlencode", "json"].includes(option)) {
      hasData = true;
      json ||= option === "json";
      if (option !== "data-raw" && option !== "data-urlencode" && v.startsWith("@")) {
        if (v.length < 2 || v === "@-" || data.length || bodyFile) return { error: { code: "conflict" } };
        bodyFile = { path: v.slice(1) };
        warnings.add("files");
      } else {
        if (bodyFile) return { error: { code: "conflict" } };
        const encoded = option === "data-urlencode" ? encodedData(v) : v;
        if (encoded === null) return { error: { code: "option", option: raw } };
        data.push(encoded);
      }
    } else if (option === "url-query") {
      const encoded = v.startsWith("+") ? v.slice(1) : encodedData(v);
      if (encoded === null) return { error: { code: "option", option: raw } };
      query.push(encoded);
    } else if (option === "form" || option === "form-string") {
      hasForm = true;
      const eq = v.indexOf("=");
      if (eq < 1) return { error: { code: "value", option: raw } };
      const name = v.slice(0, eq);
      const body = v.slice(eq + 1);
      if (option === "form-string") forms.push([name, body]);
      else if (body.startsWith("@")) {
        const parts = formFileParts(body.slice(1));
        if (!parts) return { error: { code: "value", option: raw } };
        const [path, ...attributes] = parts;
        if (!path || path === "-") return { error: { code: "value", option: raw } };
        const file: CurlFileHint = { name, path };
        for (const attribute of attributes) {
          if (attribute.startsWith("type=")) file.contentType = attribute.slice(5);
          else if (attribute.startsWith("filename=")) file.fileName = attribute.slice(9);
          else return { error: { code: "value", option: raw } };
        }
        if (file.contentType && /[\r\n]/.test(file.contentType)) return { error: { code: "value", option: raw } };
        formFiles.push(file);
        warnings.add("files");
      } else if (body.startsWith("<") || body.includes(";")) return { error: { code: "option", option: raw } };
      else forms.push([name, body]);
    }
  }
  if (!request.url) return { error: { code: "url" } };
  if ((hasForm && hasData) || (hasForm && get) || (head && hasData && !get) || (bodyFile && get)) return { error: { code: "conflict" } };
  request.method = explicitMethod || (head ? "HEAD" : get ? "GET" : hasData || hasForm ? "POST" : "GET");
  if (["GET", "HEAD"].includes(request.method) && (hasForm || (hasData && !get))) return { error: { code: "conflict" } };
  request.body = hasForm ? forms.map(([key, value]) => `${formEncode(key)}=${formEncode(value)}`).join("&") : data.join(json ? "" : "&");
  if (get && hasData) {
    query.push(request.body);
    request.body = "";
  }
  if (query.length) {
    const hash = request.url.indexOf("#");
    const fragment = hash < 0 ? "" : request.url.slice(hash);
    const base = hash < 0 ? request.url : request.url.slice(0, hash);
    request.url = `${base}${base.includes("?") ? "&" : "?"}${query.join("&")}${fragment}`;
  }
  if (auth && !request.headers.some((row) => row.name.toLowerCase() === "authorization")) request.headers.push(header("Authorization", auth));
  const cookieHeaders = request.headers.filter((row) => row.name.toLowerCase() === "cookie");
  for (const row of cookieHeaders)
    for (const pair of row.value.split(";")) {
      const eq = pair.indexOf("=");
      if (eq > 0) request.cookies.push(header(pair.slice(0, eq).trim(), pair.slice(eq + 1).trim()));
    }
  request.headers = request.headers.filter((row) => row.name.toLowerCase() !== "cookie");
  const content = [...request.headers].reverse().find((row) => row.name.toLowerCase() === "content-type")?.value;
  request.mediaType = hasForm ? "multipart/form-data" : (content ?? (json ? "application/json" : hasData && !get ? "application/x-www-form-urlencoded" : ""));
  if (json && !request.headers.some((row) => row.name.toLowerCase() === "accept")) request.headers.push(header("Accept", "application/json"));
  if (hasForm) request.headers = request.headers.filter((row) => row.name.toLowerCase() !== "content-type");
  const parsedUrl = new URL(request.url);
  if (parsedUrl.username || parsedUrl.password) {
    if (!request.headers.some((row) => row.name.toLowerCase() === "authorization")) request.headers.push(header("Authorization", `Basic ${utf8Base64(`${safeDecode(parsedUrl.username)}:${safeDecode(parsedUrl.password)}`)}`));
    parsedUrl.username = "";
    parsedUrl.password = "";
    request.url = parsedUrl.toString();
  }
  return { request, formFiles, ...(bodyFile ? { bodyFile } : {}), warnings: [...warnings] };
}

export type ParsedCurl = { url: string; method?: string; headers: WorkbenchRow[]; body: string; cookies: WorkbenchRow[] };
export function parseCurlCommand(value: string): ParsedCurl | null {
  const parsed = parseCurlImport(value);
  return "error" in parsed ? null : { url: parsed.request.url, method: parsed.request.method, headers: parsed.request.headers, cookies: parsed.request.cookies, body: parsed.request.body };
}
