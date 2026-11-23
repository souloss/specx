import { isAuthConfig, sanitizeAuthConfig, type OpenApiAuthConfig } from "./request/openapiAuth";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { uuid } from "@/lib/common/utils";

export type OpenApiParameterConfiguration = { schema?: Record<string, unknown>; style?: string; explode?: boolean; allowReserved?: boolean; location?: string };
export type WorkbenchRow = OpenApiParameterConfiguration & { name: string; value: string; enabled: boolean; type?: string; description?: string };
export type WorkbenchBulkEditParseResult = { rows: WorkbenchRow[]; error?: { line: number } };

export type WorkbenchBulkEditFormat = "colon" | "comma";

/** Standard CSV records preserve quoted delimiters, CRLF and multiline cells. */
function csvRecords(source: string): { records: Array<{ cells: string[]; line: number }>; error?: { line: number } } {
  const records: Array<{ cells: string[]; line: number }> = [];
  let cells: string[] = [],
    cell = "",
    quoted = false,
    closed = false,
    line = 1,
    start = 1;
  for (let i = 0; i <= source.length; i++) {
    const c = source[i];
    if (quoted) {
      if (c === undefined) return { records, error: { line: start } };
      if (c === '"') {
        if (source[i + 1] === '"') {
          cell += '"';
          i++;
        } else {
          quoted = false;
          closed = true;
        }
      } else {
        cell += c;
        if (c === "\n") line++;
      }
    } else if (c === "," || c === "\n" || c === undefined || (c === "\r" && source[i + 1] === "\n")) {
      cells.push(cell);
      cell = "";
      closed = false;
      if (c !== ",") {
        if (cells.some((value) => value.trim())) records.push({ cells, line: start });
        cells = [];
        if (c === "\r") i++;
        line++;
        start = line;
      }
    } else if (c === '"' && !cell && !closed) quoted = true;
    else if (closed || c === '"') return { records, error: { line: start } };
    else cell += c;
  }
  return { records };
}
export function parseWorkbenchBulkEdit(value: string, format: WorkbenchBulkEditFormat = "colon"): WorkbenchBulkEditParseResult {
  const rows: WorkbenchRow[] = [];
  if (format === "comma") {
    const parsed = csvRecords(value);
    for (const record of parsed.records) {
      const [enabled, name, content, type, description] = record.cells;
      if (record.cells.length !== 5 || !name?.trim() || !["true", "false", "1", "0"].includes(enabled!.toLowerCase())) return { rows, error: { line: record.line } };
      rows.push({ enabled: enabled === "1" || enabled?.toLowerCase() === "true", name: name.trim(), value: content!, type: type!, description: description! });
    }
    return { rows, ...(parsed.error ? { error: parsed.error } : {}) };
  }
  for (const [index, source] of value.split(/\r?\n/).entries()) {
    if (!source.trim()) continue;
    const enabled = !source.trimStart().startsWith("#");
    const content = enabled ? source.trimStart() : source.trimStart().slice(1).trimStart();
    const delimiter = content.indexOf(":");
    if (delimiter < 1 || !content.slice(0, delimiter).trim()) return { rows, error: { line: index + 1 } };
    rows.push({ name: content.slice(0, delimiter).trim(), value: content.slice(delimiter + 1).trimStart(), enabled });
  }
  return { rows };
}
export function formatWorkbenchBulkEdit(rows: WorkbenchRow[], format: WorkbenchBulkEditFormat = "colon"): string {
  const quote = (value: string) => (/[",\r\n]/u.test(value) ? `"${value.replaceAll('"', '""')}"` : value);
  return rows.map((row) => (format === "comma" ? [String(row.enabled), row.name, row.value, row.type ?? "string", row.description ?? ""].map(quote).join(",") : `${row.enabled ? "" : "# "}${row.name}: ${row.value}`)).join("\n");
}

export type WorkbenchRequest = {
  method: string;
  url: string;
  headers: WorkbenchRow[];
  cookies: WorkbenchRow[];
  body: string;
  mediaType: string;
  form?: WorkbenchRow[];
  disabledHeaders?: string[];
  auth?: OpenApiAuthConfig;
};

export type WorkbenchResponse = {
  status: number;
  statusText: string;
  duration: number;
  body: string;
  headers: Array<[string, string]>;
  truncated: boolean;
  scriptLogs?: string[];
  scriptError?: string;
};

export type WorkbenchHistoryEntry = {
  id: string;
  serviceId: string;
  operationKey: string;
  operationLabel: string;
  request: WorkbenchRequest;
  response: Pick<WorkbenchResponse, "status" | "statusText" | "duration"> | null;
  createdAt: string;
};

const HISTORY_LIMIT = 30;
const HISTORY_KEY_PREFIX = "specx-openapi-history-v1:";
const SENSITIVE_NAME = /(?:authorization|cookie|password|passwd|secret|token|api[-_]?key|credential|signature)/i;

function storageKey(prefix: string, serviceId: string): string {
  return `${prefix}${serviceId}`;
}

function isRow(value: unknown): value is WorkbenchRow {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const row = value as Record<string, unknown>;
  return typeof row.name === "string" && typeof row.value === "string" && typeof row.enabled === "boolean";
}

function isRequest(value: unknown): value is WorkbenchRequest {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const request = value as Record<string, unknown>;
  return (
    typeof request.method === "string" &&
    typeof request.url === "string" &&
    Array.isArray(request.headers) &&
    request.headers.every(isRow) &&
    Array.isArray(request.cookies) &&
    request.cookies.every(isRow) &&
    typeof request.body === "string" &&
    typeof request.mediaType === "string" &&
    (request.auth === undefined || isAuthConfig(request.auth))
  );
}

export { isRequest as isWorkbenchRequest };

function parseList<T>(raw: string | null, guard: (value: unknown) => value is T): T[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter(guard) : [];
  } catch {
    return [];
  }
}

export function loadWorkbenchHistory(serviceId: string): WorkbenchHistoryEntry[] {
  return parseList<WorkbenchHistoryEntry>(safeLocalStorageGet(storageKey(HISTORY_KEY_PREFIX, serviceId)), (value): value is WorkbenchHistoryEntry => {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const entry = value as Record<string, unknown>;
    return (
      typeof entry.id === "string" &&
      entry.serviceId === serviceId &&
      typeof entry.operationKey === "string" &&
      typeof entry.operationLabel === "string" &&
      isRequest(entry.request) &&
      (entry.response === null || (typeof entry.response === "object" && typeof (entry.response as Record<string, unknown>).status === "number")) &&
      typeof entry.createdAt === "string"
    );
  });
}

export function appendWorkbenchHistory(serviceId: string, entry: Omit<WorkbenchHistoryEntry, "id" | "serviceId" | "createdAt">): WorkbenchHistoryEntry[] {
  const next: WorkbenchHistoryEntry = { ...entry, id: uuid(), serviceId, createdAt: new Date().toISOString(), request: redactRequest(entry.request) };
  const history = [next, ...loadWorkbenchHistory(serviceId)].slice(0, HISTORY_LIMIT);
  safeLocalStorageSet(storageKey(HISTORY_KEY_PREFIX, serviceId), JSON.stringify(history));
  return history;
}

export function removeWorkbenchHistory(serviceId: string, id: string): WorkbenchHistoryEntry[] {
  const history = loadWorkbenchHistory(serviceId).filter((entry) => entry.id !== id);
  safeLocalStorageSet(storageKey(HISTORY_KEY_PREFIX, serviceId), JSON.stringify(history));
  return history;
}

export function clearWorkbenchHistory(serviceId: string): void {
  safeLocalStorageSet(storageKey(HISTORY_KEY_PREFIX, serviceId), "[]");
}

function redactBody(body: string): string {
  return body.replace(/("(?:password|passwd|secret|token|api[-_]?key|authorization|cookie|credential|signature)"\s*:\s*)"(?:[^"\\]|\\.)*"/gi, '$1"{{redacted}}"');
}

function redactUrl(url: string): string {
  try {
    const parsed = new URL(url, globalThis.location?.origin ?? "http://localhost");
    for (const [key] of parsed.searchParams) if (SENSITIVE_NAME.test(key)) parsed.searchParams.set(key, "{{redacted}}");
    return parsed.href;
  } catch {
    return url;
  }
}

export function redactRequest(request: WorkbenchRequest): WorkbenchRequest {
  return {
    ...request,
    url: redactUrl(request.url),
    headers: request.headers.map((row) => ({ ...row, value: SENSITIVE_NAME.test(row.name) ? "{{redacted}}" : row.value })),
    cookies: request.cookies.map((row) => ({ ...row, value: SENSITIVE_NAME.test(row.name) ? "{{redacted}}" : row.value })),
    body: redactBody(request.body),
    ...(request.auth ? { auth: sanitizeAuthConfig(request.auth) } : {}),
  };
}

export { parseCurlCommand } from "./request/openapiCurl";

export { buildCodeSnippet, buildCurlCommand, shellQuote, type SnippetLanguage } from "../snippets/openapiCodegen";
