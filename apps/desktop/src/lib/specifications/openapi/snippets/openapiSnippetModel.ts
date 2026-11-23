import type { WorkbenchRequest } from "../workbench/openapiWorkbench";

export type SnippetFile = { path: string; fileName?: string; contentType?: string };
export type SnippetFormPart = { name: string; value: string } | { name: string; file: SnippetFile };
export type SnippetOptions = { timeoutMs?: number; form?: SnippetFormPart[]; file?: SnippetFile };

export function normalizeSnippetRequest(request: WorkbenchRequest, options: SnippetOptions) {
  const method = request.method.toUpperCase();
  const hasBody = !["GET", "HEAD"].includes(method);
  const mediaType = request.mediaType.split(";")[0]!.trim().toLowerCase();
  const multipart = hasBody && mediaType === "multipart/form-data";
  const form: SnippetFormPart[] = multipart ? (options.form ?? Array.from(new URLSearchParams(request.body), ([name, value]) => ({ name, value }))) : [];
  const file = hasBody ? options.file : undefined;
  let body = hasBody && !multipart && !file ? request.body : "";
  const emptyMultipart = multipart && !form.length;
  const headers = request.headers.filter((row) => row.enabled && row.name.trim() && !["content-length", "host"].includes(row.name.trim().toLowerCase())).map((row) => ({ name: row.name.trim(), value: row.value }));
  const cookies = request.cookies
    .filter((row) => row.enabled && row.name.trim())
    .map((row) => `${row.name.trim()}=${row.value}`)
    .join("; ");
  const cookieHeader = headers.find((row) => row.name.toLowerCase() === "cookie");
  if (cookies) {
    if (cookieHeader) cookieHeader.value = `${cookieHeader.value}; ${cookies}`;
    else headers.push({ name: "Cookie", value: cookies });
  }
  if (multipart) {
    for (let i = headers.length - 1; i >= 0; i -= 1) if (headers[i]!.name.toLowerCase() === "content-type") headers.splice(i, 1);
    if (emptyMultipart) {
      headers.push({ name: "Content-Type", value: "multipart/form-data; boundary=SpecX-empty" });
      body = "--SpecX-empty--\r\n";
    }
  } else if (hasBody && request.mediaType && !headers.some((row) => row.name.toLowerCase() === "content-type")) headers.push({ name: "Content-Type", value: request.mediaType });
  const timeoutMs = Math.round(Math.max(1, Math.min(300_000, Number.isFinite(options.timeoutMs) ? options.timeoutMs! : 20_000)));
  return { method, headers, body, multipart: multipart && !emptyMultipart, form, file, timeoutMs, seconds: timeoutMs / 1000 };
}

export type SnippetRequestModel = ReturnType<typeof normalizeSnippetRequest>;
