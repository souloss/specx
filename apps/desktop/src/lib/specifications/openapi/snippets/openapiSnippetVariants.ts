import type { WorkbenchRequest } from "../workbench/openapiWorkbench";
import type { SnippetRequestModel } from "./openapiSnippetModel";
import type { SnippetLanguage } from "./openapiSnippetClients";

const js = (value: string) => JSON.stringify(value).replaceAll("\u2028", "\\u2028").replaceAll("\u2029", "\\u2029");
const sh = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const ps = (value: string) => `'${value.replaceAll("'", "''")}'`;
// Java processes Unicode escapes before tokenization. Keep literal backslashes
// escaped and control characters octal, just as in the JDK HttpClient generator.
function java(value: string): string {
  const escapes: Record<string, string> = { '"': '\\"', "\\": "\\\\", "\n": "\\n", "\r": "\\r", "\t": "\\t", "\b": "\\b", "\f": "\\f" };
  return '"' + Array.from(value, (char) => escapes[char] ?? (char.charCodeAt(0) < 32 ? `\\${char.charCodeAt(0).toString(8).padStart(3, "0")}` : char)).join("") + '"';
}
function filename(path: string): string {
  return path.split(/[\\/]/).at(-1) || "upload.bin";
}
function disposition(value: string): string {
  return value.replaceAll('"', "%22").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
}

function javascriptBody(m: SnippetRequestModel, browser: boolean): { setup: string[]; body: string } {
  const setup: string[] = [];
  let body = m.body ? js(m.body) : "undefined";
  let fileIndex = 0;
  function file(path: string): string {
    if (!browser) return `new Blob([await readFile(${js(path)})])`;
    const name = `file${fileIndex}`;
    setup.push(`const ${name} = document.querySelector('input[type="file"]')?.files?.[${fileIndex++}];`, `if (!${name}) throw new Error("Select the required local file first");`);
    return name;
  }
  if (m.multipart) {
    setup.push("const body = new FormData();");
    for (const part of m.form) {
      if ("value" in part) setup.push(`body.append(${js(part.name)}, ${js(part.value)});`);
      else setup.push(`body.append(${js(part.name)}, new Blob([${file(part.file.path)}], { type: ${js(part.file.contentType || "application/octet-stream")} }), ${js(part.file.fileName || filename(part.file.path))});`);
    }
    body = "body";
  } else if (m.file) body = browser ? file(m.file.path) : `await readFile(${js(m.file.path)})`;
  return { setup, body };
}

function javascript(request: WorkbenchRequest, language: SnippetLanguage, m: SnippetRequestModel): string {
  const browser = language === "xhr" || language === "jquery";
  const { setup, body } = javascriptBody(m, browser);
  const headers = JSON.stringify(Object.fromEntries(m.headers.map((row) => [row.name, row.value])), null, 2);
  if (browser) {
    if (language === "xhr")
      return [
        ...setup,
        "const xhr = new XMLHttpRequest();",
        `xhr.open(${js(m.method)}, ${js(request.url)});`,
        `xhr.timeout = ${m.timeoutMs};`,
        "xhr.withCredentials = false;",
        ...m.headers.map((row) => `xhr.setRequestHeader(${js(row.name)}, ${js(row.value)});`),
        "xhr.onload = () => console.log(xhr.status, xhr.responseText);",
        'xhr.onerror = () => console.error("Network or CORS error");',
        'xhr.ontimeout = () => console.error("Request timed out");',
        `xhr.send(${body});`,
      ].join("\n");
    return [
      "// Load jQuery in the page before running this example.",
      ...setup,
      `$.ajax({\n  url: ${js(request.url)},\n  method: ${js(m.method)},\n  headers: ${headers},\n  data: ${body},\n  processData: false,\n  contentType: false,\n  timeout: ${m.timeoutMs},\n  xhrFields: { withCredentials: false },\n}).done((data, status, xhr) => console.log(xhr.status, data))\n  .fail((xhr, status, error) => console.error(xhr.status, status, error));`,
    ].join("\n\n");
  }
  // Serialize native FormData once for clients that accept a raw byte body.
  // This preserves duplicate fields and lets the serializer own its boundary.
  const imports = [m.file || m.form.some((part) => "file" in part) ? 'import { readFile } from "node:fs/promises";' : ""];
  const lines = [
    ...setup,
    `const payload = ${body};`,
    `const prepared = new Request(${js(request.url)}, { method: ${js(m.method)}, headers: ${headers}, body: payload });`,
    "const headers = Object.fromEntries(prepared.headers);",
    "const bytes = payload === undefined ? undefined : Buffer.from(await prepared.arrayBuffer());",
  ];
  if (language === "node") {
    imports.unshift(`import { request } from ${js(new URL(request.url, "https://api.example.com").protocol === "http:" ? "node:http" : "node:https")};`);
    lines.push(
      `const req = request(${js(request.url)}, { method: ${js(m.method)}, headers }, (res) => {\n  const chunks = [];\n  res.on("data", (chunk) => chunks.push(chunk));\n  res.on("end", () => console.log(res.statusCode, Buffer.concat(chunks).toString("utf8")));\n});`,
      `const timer = setTimeout(() => req.destroy(new Error("Request timed out")), ${m.timeoutMs});`,
      'req.on("close", () => clearTimeout(timer));',
      'req.on("error", console.error);',
      "req.end(bytes);",
    );
  } else if (language === "request") {
    imports.unshift('import request from "request";');
    lines.push(`request({ url: ${js(request.url)}, method: ${js(m.method)}, headers, body: bytes, timeout: ${m.timeoutMs} }, (error, response, body) => {\n  if (error) console.error(error);\n  else console.log(response.statusCode, body);\n});`);
  } else {
    imports.unshift('import unirest from "unirest";');
    // Unirest's public API exposes the underlying request options. A raw body
    // avoids its object/form guessing and preserves UTF-8 and multipart bytes.
    lines.push(
      `const req = unirest(${js(m.method)}, ${js(request.url)}).headers(headers).timeout(${m.timeoutMs});`,
      "req.options.body = bytes;",
      "req.end((response) => {\n  if (response.error && !response.status) console.error(response.error);\n  else console.log(response.status, response.raw_body);\n});",
    );
  }
  return [...imports.filter(Boolean), ...lines].join("\n\n");
}

function powershell(request: WorkbenchRequest, m: SnippetRequestModel): string {
  const lines = [
    "Add-Type -AssemblyName System.Net.Http",
    "$client = [System.Net.Http.HttpClient]::new()",
    `$client.Timeout = [TimeSpan]::FromMilliseconds(${m.timeoutMs})`,
    `$request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::new(${ps(m.method)}), ${ps(request.url)})`,
  ];
  if (m.multipart) {
    lines.push("$body = [System.Net.Http.MultipartFormDataContent]::new()");
    m.form.forEach((part, index) => {
      if ("value" in part) lines.push(`$body.Add([System.Net.Http.StringContent]::new(${ps(part.value)}, [Text.Encoding]::UTF8), ${ps(part.name)})`);
      else
        lines.push(
          `$file${index} = [System.Net.Http.ByteArrayContent]::new([IO.File]::ReadAllBytes(${ps(part.file.path)}))`,
          `$file${index}.Headers.ContentType = [System.Net.Http.Headers.MediaTypeHeaderValue]::new(${ps(part.file.contentType || "application/octet-stream")})`,
          `$body.Add($file${index}, ${ps(part.name)}, ${ps(part.file.fileName || filename(part.file.path))})`,
        );
    });
    lines.push("$request.Content = $body");
  } else if (m.file) lines.push(`$request.Content = [System.Net.Http.ByteArrayContent]::new([IO.File]::ReadAllBytes(${ps(m.file.path)}))`);
  else if (m.body) lines.push(`$request.Content = [System.Net.Http.ByteArrayContent]::new([Text.Encoding]::UTF8.GetBytes(${ps(m.body)}))`);
  for (const row of m.headers) {
    if (row.name.toLowerCase().startsWith("content-")) {
      if (!m.body && !m.file && !m.multipart) lines.push("if ($null -eq $request.Content) { $request.Content = [System.Net.Http.ByteArrayContent]::new([byte[]]@()) }");
      lines.push(`$request.Content.Headers.TryAddWithoutValidation(${ps(row.name)}, ${ps(row.value)}) | Out-Null`);
    } else lines.push(`$request.Headers.TryAddWithoutValidation(${ps(row.name)}, ${ps(row.value)}) | Out-Null`);
  }
  lines.push("try {", "  $response = $client.SendAsync($request).GetAwaiter().GetResult()", "  try {", "    [int]$response.StatusCode", "    $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()", "  } finally { $response.Dispose() }", "} finally { $request.Dispose(); $client.Dispose() }");
  return lines.join("\n");
}

function shellMultipartBody(m: SnippetRequestModel): string[] {
  // A raw body avoids HTTPie's field grammar interpreting a literal =@ as
  // a local file, and preserves file paths containing semicolons.
  const setup = ["payload=$(mktemp)", "trap 'rm -f \"$payload\"' EXIT HUP INT TERM", "boundary=\"SpecX-$(od -An -N16 -tx1 /dev/urandom | tr -d ' \\n')\"", "{"];
  for (const part of m.form) {
    const prefix = `Content-Disposition: form-data; name="${disposition(part.name)}"${"file" in part ? `; filename="${disposition(part.file.fileName || filename(part.file.path))}"` : ""}\r\n${"file" in part ? `Content-Type: ${part.file.contentType || "application/octet-stream"}\r\n` : ""}\r\n`;
    setup.push(`  printf '%s\\r\\n' "--$boundary"`, `  printf '%s' ${sh(prefix)}`, "value" in part ? `  printf '%s' ${sh(part.value)}` : `  cat -- ${sh(part.file.path)}`, "  printf '\\r\\n'");
  }
  setup.push("  printf '%s--\\r\\n' \"--$boundary\"", '} > "$payload"');
  return setup;
}

function shell(request: WorkbenchRequest, language: SnippetLanguage, m: SnippetRequestModel): string {
  if (language === "powershell") return powershell(request, m);
  if (language === "curlWindows") {
    const args = ["--request", m.method, "--max-time", String(m.seconds), request.url];
    for (const row of m.headers) args.push("--header", `${row.name}: ${row.value}`);
    if (m.multipart)
      for (const part of m.form) {
        if ("value" in part) args.push("--form-string", `${part.name}=${part.value}`);
        else {
          const quote = (value: string) => '"' + value.replaceAll("\\", "\\\\").replaceAll('"', '\\"') + '"';
          args.push("--form", `${part.name}=@${quote(part.file.path)}${part.file.contentType ? `;type=${part.file.contentType}` : ""}${part.file.fileName ? `;filename=${quote(part.file.fileName)}` : ""}`);
        }
      }
    else if (m.file) args.push("--data-binary", `@${m.file.path}`);
    else if (m.body) args.push("--data-raw", m.body);
    return `# PowerShell 7.3+ preserves native argument quoting.\n& curl.exe ${args.map(ps).join(" `\n  ")}`;
  }
  if (language === "httpie") {
    const escape = (value: string) => value.replace(/[\\:=@;]/g, "\\$&");
    const args = ["http", `--timeout=${m.seconds}`, sh(m.method), sh(request.url), ...m.headers.map((row) => sh(`${escape(row.name)}:${escape(row.value)}`))];
    if (m.multipart) return ["set -e", ...shellMultipartBody(m), `${args.join(" \\\n  ")} "Content-Type:multipart/form-data; boundary=$boundary" < "$payload"`].join("\n");
    if (m.file) return `${args.join(" \\\n  ")} < ${sh(m.file.path)}`;
    if (m.body) return `printf '%s' ${sh(m.body)} | ${args.join(" \\\n  ")}`;
    return [...args.slice(0, 1), "--ignore-stdin", ...args.slice(1)].join(" \\\n  ");
  }
  const setup: string[] = [];
  const args = ["wget", "--output-document=-", `--timeout=${m.seconds}`, "--tries=1", `--method=${sh(m.method)}`, ...m.headers.map((row) => `--header=${sh(`${row.name}: ${row.value}`)}`)];
  if (m.multipart) {
    setup.push(...shellMultipartBody(m));
    args.push('--header="Content-Type: multipart/form-data; boundary=$boundary"', '--body-file="$payload"');
  } else if (m.file) args.push(`--body-file=${sh(m.file.path)}`);
  else if (m.body) args.push(`--body-data=${sh(m.body)}`);
  args.push(sh(request.url));
  return [...(m.multipart ? ["set -e"] : []), ...setup, args.join(" \\\n  ")].join("\n");
}

function javaClient(request: WorkbenchRequest, language: SnippetLanguage, m: SnippetRequestModel): string {
  const setup: string[] = [];
  let payload = m.body ? `${java(m.body)}.getBytes(StandardCharsets.UTF_8)` : "new byte[0]";
  if (m.file) payload = `Files.readAllBytes(Path.of(${java(m.file.path)}))`;
  if (m.multipart) {
    setup.push('String boundary = "SpecX-" + UUID.randomUUID();', "ByteArrayOutputStream payload = new ByteArrayOutputStream();");
    for (const part of m.form) {
      const prefix = `\r\nContent-Disposition: form-data; name="${disposition(part.name)}"${"file" in part ? `; filename="${disposition(part.file.fileName || filename(part.file.path))}"` : ""}\r\n${"file" in part ? `Content-Type: ${part.file.contentType || "application/octet-stream"}\r\n` : ""}\r\n`;
      setup.push(
        `payload.write(("--" + boundary + ${java(prefix)}).getBytes(StandardCharsets.UTF_8));`,
        `payload.write(${"value" in part ? `${java(part.value)}.getBytes(StandardCharsets.UTF_8)` : `Files.readAllBytes(Path.of(${java(part.file.path)}))`});`,
        'payload.write("\\r\\n".getBytes(StandardCharsets.UTF_8));',
      );
    }
    setup.push('payload.write(("--" + boundary + "--\\r\\n").getBytes(StandardCharsets.UTF_8));');
    payload = "payload.toByteArray()";
  }
  const headers = [...m.headers.map((row) => `.header(${java(row.name)}, ${java(row.value)})`), ...(m.multipart ? ['.header("Content-Type", "multipart/form-data; boundary=" + boundary)'] : [])];
  const imports = "import java.io.*;\nimport java.nio.charset.StandardCharsets;\nimport java.nio.file.*;\nimport java.util.*;";
  const hasBody = !["GET", "HEAD"].includes(m.method);
  const code =
    language === "okhttp"
      ? [
          `OkHttpClient client = new OkHttpClient.Builder().callTimeout(${m.timeoutMs}, java.util.concurrent.TimeUnit.MILLISECONDS).build();`,
          `Request request = new Request.Builder().url(${java(request.url)})`,
          ...headers.map((line) => `    ${line}`),
          `    .method(${java(m.method)}, ${hasBody ? `RequestBody.create(${payload}, (MediaType) null)` : "null"}).build();`,
          "try (Response response = client.newCall(request).execute()) {",
          "  System.out.println(response.code());",
          '  System.out.println(response.body() == null ? "" : response.body().string());',
          "} finally { client.dispatcher().executorService().shutdown(); client.connectionPool().evictAll(); }",
        ]
      : [
          `Unirest.config().connectTimeout(${m.timeoutMs}).requestTimeout(${m.timeoutMs});`,
          "try {",
          `  HttpResponse<String> response = Unirest.request(${java(m.method)}, ${java(request.url)})`,
          ...headers.map((line) => `      ${line}`),
          ...(hasBody ? [`      .body(${payload})`] : []),
          "      .asString();",
          "  System.out.println(response.getStatus());",
          "  System.out.println(response.getBody());",
          "} finally { Unirest.shutDown(); }",
        ];
  return `${language === "okhttp" ? "import okhttp3.*;" : "import kong.unirest.core.Unirest;\nimport kong.unirest.core.HttpResponse;"}\n${imports}\n\npublic class SpecXRequest {\n  public static void main(String[] args) throws Exception {\n${[...setup, ...code].map((line) => `    ${line}`).join("\n")}\n  }\n}`;
}

export function buildAdditionalSnippet(request: WorkbenchRequest, language: SnippetLanguage, model: SnippetRequestModel): string | undefined {
  if (["xhr", "jquery", "node", "request", "unirest"].includes(language)) return javascript(request, language, model);
  if (["curlWindows", "httpie", "wget", "powershell"].includes(language)) return shell(request, language, model);
  if (["okhttp", "javaUnirest"].includes(language)) return javaClient(request, language, model);
}
