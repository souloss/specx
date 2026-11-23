import type { WorkbenchRequest } from "../workbench/openapiWorkbench";
import { normalizeSnippetRequest, type SnippetOptions } from "./openapiSnippetModel";
import { buildAdditionalSnippet } from "./openapiSnippetVariants";
import type { SnippetLanguage } from "./openapiSnippetClients";
export type { SnippetLanguage } from "./openapiSnippetClients";
export type { SnippetFile, SnippetFormPart, SnippetOptions } from "./openapiSnippetModel";

export function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}
const js = (value: string) => JSON.stringify(value).replaceAll("\u2028", "\\u2028").replaceAll("\u2029", "\\u2029");
const py = (value: string) => JSON.stringify(value);
const single = (value: string) => `'${value.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`;
const ruby = (value: string) => js(value).replaceAll("#", "\\#");
const csharp = (value: string) => js(value);
function go(value: string): string {
  return js(value);
}
function java(value: string): string {
  const escapes: Record<string, string> = { '"': '\\"', "\\": "\\\\", "\n": "\\n", "\r": "\\r", "\t": "\\t", "\b": "\\b", "\f": "\\f" };
  return '"' + Array.from(value, (char) => escapes[char] ?? (char.charCodeAt(0) < 32 ? `\\${char.charCodeAt(0).toString(8).padStart(3, "0")}` : char)).join("") + '"';
}
function php(value: string): string {
  return single(value);
}

function curlFormQuote(value: string): string {
  return '"' + value.replaceAll("\\", "\\\\").replaceAll('"', '\\"') + '"';
}

export function buildCurlCommand(request: WorkbenchRequest, options: SnippetOptions = {}): string {
  const m = normalizeSnippetRequest(request, options);
  const lines = [`curl --request ${shellQuote(m.method)} --max-time ${m.seconds} ${shellQuote(request.url)}`];
  for (const header of m.headers) lines.push(`  --header ${shellQuote(`${header.name}: ${header.value}`)}`);
  if (m.multipart)
    for (const part of m.form) {
      if ("value" in part) lines.push(`  --form-string ${shellQuote(`${part.name}=${part.value}`)}`);
      else lines.push(`  --form ${shellQuote(`${part.name}=@${curlFormQuote(part.file.path)}${part.file.contentType ? `;type=${part.file.contentType}` : ""}${part.file.fileName ? `;filename=${curlFormQuote(part.file.fileName)}` : ""}`)}`);
    }
  else if (m.file) lines.push(`  --data-binary ${shellQuote(`@${m.file.path}`)}`);
  else if (m.body) lines.push(`  --data-raw ${shellQuote(m.body)}`);
  return lines.join(" \\\n");
}

export function buildCodeSnippet(request: WorkbenchRequest, language: SnippetLanguage, options: SnippetOptions = {}): string {
  if (language === "curl") return buildCurlCommand(request, options);
  const m = normalizeSnippetRequest(request, options);
  const additional = buildAdditionalSnippet(request, language, m);
  if (additional !== undefined) return additional;
  if (language === "fetch" || language === "axios") {
    const imports = [language === "axios" ? 'import axios from "axios";' : "", m.file || m.form.some((part) => "file" in part) ? 'import { readFile } from "node:fs/promises";' : ""].filter(Boolean);
    const setup: string[] = [];
    let body = m.body ? js(m.body) : "";
    if (m.multipart) {
      setup.push("const body = new FormData();");
      m.form.forEach((part) =>
        setup.push(
          "value" in part
            ? `body.append(${js(part.name)}, ${js(part.value)});`
            : `body.append(${js(part.name)}, new Blob([await readFile(${js(part.file.path)})], { type: ${js(part.file.contentType || "application/octet-stream")} }), ${js(part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin")});`,
        ),
      );
      body = "body";
    } else if (m.file) {
      setup.push(`const body = await readFile(${js(m.file.path)});`);
      body = "body";
    }
    const headers = JSON.stringify(Object.fromEntries(m.headers.map((row) => [row.name, row.value])), null, 2);
    const call =
      language === "fetch"
        ? `const response = await fetch(${js(request.url)}, {\n  method: ${js(m.method)},\n  headers: ${headers},${body ? `\n  body: ${body},` : ""}\n  signal: AbortSignal.timeout(${m.timeoutMs}),\n});\nconsole.log(response.status, await response.text());`
        : `const response = await axios({\n  method: ${js(m.method)},\n  url: ${js(request.url)},\n  headers: ${headers},${body ? `\n  data: ${body},` : ""}\n  timeout: ${m.timeoutMs},\n  validateStatus: () => true,\n});\nconsole.log(response.status, response.data);`;
    return [...imports, ...setup, call].join("\n\n");
  }
  if (language === "python") {
    const lines = ["import requests", "from contextlib import ExitStack", "", "with ExitStack() as stack:"];
    let data = m.body ? `${py(m.body)}.encode("utf-8")` : "";
    if (m.file) data = `stack.enter_context(open(${py(m.file.path)}, "rb"))`;
    const files = m.multipart
      ? m.form.map((part) =>
          "value" in part ? `(${py(part.name)}, (None, ${py(part.value)}))` : `(${py(part.name)}, (${py(part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin")}, stack.enter_context(open(${py(part.file.path)}, "rb")), ${py(part.file.contentType || "application/octet-stream")}))`,
        )
      : [];
    lines.push(
      "    response = requests.request(",
      `        ${py(m.method)}, ${py(request.url)},`,
      `        headers={${m.headers.map((row) => `${py(row.name)}: ${py(row.value)}`).join(", ")}},`,
      ...(files.length ? [`        files=[${files.join(",\n               ")}],`] : data ? [`        data=${data},`] : []),
      `        timeout=${m.seconds},`,
      "    )",
      "    print(response.status_code)",
      "    print(response.text)",
    );
    return lines.join("\n");
  }
  if (language === "go") {
    const imports = ['"fmt"', '"io"', '"net/http"', '"time"', ...(m.multipart ? ['"bytes"', '"mime/multipart"'] : m.body ? ['"strings"'] : []), ...(m.file || m.form.some((part) => "file" in part) ? ['"os"'] : []), ...(m.form.some((part) => "file" in part) ? ['"net/textproto"', '"mime"'] : [])];
    const setup: string[] = [];
    let body = m.body ? `strings.NewReader(${go(m.body)})` : "nil";
    if (m.multipart) {
      setup.push("var payload bytes.Buffer", "writer := multipart.NewWriter(&payload)");
      m.form.forEach((part, index) => {
        if ("value" in part) setup.push(`if err := writer.WriteField(${go(part.name)}, ${go(part.value)}); err != nil { panic(err) }`);
        else
          setup.push(
            `file${index}, err := os.Open(${go(part.file.path)})`,
            "if err != nil { panic(err) }",
            `defer file${index}.Close()`,
            `headers${index} := make(textproto.MIMEHeader)`,
            `headers${index}.Set("Content-Disposition", mime.FormatMediaType("form-data", map[string]string{"name": ${go(part.name)}, "filename": ${go(part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin")}}))`,
            `headers${index}.Set("Content-Type", ${go(part.file.contentType || "application/octet-stream")})`,
            `part${index}, err := writer.CreatePart(headers${index})`,
            "if err != nil { panic(err) }",
            `if _, err = io.Copy(part${index}, file${index}); err != nil { panic(err) }`,
          );
      });
      setup.push("if err := writer.Close(); err != nil { panic(err) }");
      body = "&payload";
    } else if (m.file) {
      setup.push(`payload, err := os.Open(${go(m.file.path)})`, "if err != nil { panic(err) }", "defer payload.Close()");
      body = "payload";
    }
    return `package main\n\nimport (\n${imports.map((item) => `  ${item}`).join("\n")}\n)\n\nfunc main() {\n${[...setup, `req, err := http.NewRequest(${go(m.method)}, ${go(request.url)}, ${body})`, "if err != nil { panic(err) }", ...m.headers.map((row) => `req.Header.Add(${go(row.name)}, ${go(row.value)})`), ...(m.multipart ? ['req.Header.Set("Content-Type", writer.FormDataContentType())'] : []), `client := &http.Client{Timeout: ${m.timeoutMs} * time.Millisecond}`, "resp, err := client.Do(req)", "if err != nil { panic(err) }", "defer resp.Body.Close()", "body, err := io.ReadAll(resp.Body)", "if err != nil { panic(err) }", "fmt.Println(resp.Status, string(body))"].map((line) => `  ${line}`).join("\n")}\n}`;
  }
  if (language === "java") {
    const setup: string[] = [];
    let publisher = m.body ? `HttpRequest.BodyPublishers.ofString(${java(m.body)}, StandardCharsets.UTF_8)` : "HttpRequest.BodyPublishers.noBody()";
    if (m.file) publisher = `HttpRequest.BodyPublishers.ofFile(Path.of(${java(m.file.path)}))`;
    if (m.multipart) {
      setup.push('String boundary = "SpecX-" + UUID.randomUUID();', "List<byte[]> parts = new ArrayList<>();");
      for (const part of m.form) {
        const filename = "file" in part ? part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin" : undefined;
        const escapedName = part.name.replaceAll('"', "%22").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
        const escapedFile = filename?.replaceAll('"', "%22").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
        const disposition = `\r\nContent-Disposition: form-data; name="${escapedName}"${escapedFile ? `; filename="${escapedFile}"` : ""}\r\n${"file" in part ? `Content-Type: ${part.file.contentType || "application/octet-stream"}\r\n` : ""}\r\n`;
        setup.push(
          `parts.add(("--" + boundary + ${java(disposition)}).getBytes(StandardCharsets.UTF_8));`,
          `parts.add(${"value" in part ? `${java(part.value)}.getBytes(StandardCharsets.UTF_8)` : `Files.readAllBytes(Path.of(${java(part.file.path)}))`});`,
          'parts.add("\\r\\n".getBytes(StandardCharsets.UTF_8));',
        );
      }
      setup.push('parts.add(("--" + boundary + "--\\r\\n").getBytes(StandardCharsets.UTF_8));');
      publisher = "HttpRequest.BodyPublishers.ofByteArrays(parts)";
    }
    return `import java.net.URI;\nimport java.net.http.*;\nimport java.nio.charset.StandardCharsets;\nimport java.nio.file.*;\nimport java.time.Duration;\nimport java.util.*;\n\npublic class SpecXRequest {\n  public static void main(String[] args) throws Exception {\n${setup.map((line) => `    ${line}`).join("\n")}\n    HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofMillis(${m.timeoutMs})).build();\n    HttpRequest request = HttpRequest.newBuilder()\n        .uri(URI.create(${java(request.url)}))\n        .timeout(Duration.ofMillis(${m.timeoutMs}))\n        .method(${java(m.method)}, ${publisher})\n${m.headers.map((row) => `        .header(${java(row.name)}, ${java(row.value)})`).join("\n")}${m.multipart ? '\n        .header("Content-Type", "multipart/form-data; boundary=" + boundary)' : ""}\n        .build();\n    HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());\n    System.out.println(response.statusCode());\n    System.out.println(response.body());\n  }\n}`;
  }
  if (language === "php") {
    let body = m.body ? php(m.body) : "";
    const setup: string[] = [];
    if (m.multipart) {
      // A multipart body is assembled explicitly so duplicate field names stay intact.
      setup.push("$boundary = 'SpecX-' . bin2hex(random_bytes(12));", "$body = '';");
      m.form.forEach((part) => {
        const filename = "file" in part ? part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin" : undefined;
        const escapedName = part.name.replaceAll('"', "%22").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
        const prefix = `Content-Disposition: form-data; name="${escapedName}"${filename ? `; filename="${filename.replaceAll('"', "%22").replaceAll("\r", "%0D").replaceAll("\n", "%0A")}"` : ""}`;
        setup.push(
          `$body .= '--' . $boundary . "\\r\\n" . ${php(prefix)} . "\\r\\n"${"file" in part ? ` . ${php(`Content-Type: ${part.file.contentType || "application/octet-stream"}`)} . "\\r\\n"` : ""} . "\\r\\n";`,
          `$body .= ${"value" in part ? php(part.value) : `file_get_contents(${php(part.file.path)})`} . "\\r\\n";`,
        );
      });
      setup.push("$body .= '--' . $boundary . \"--\\r\\n\";");
      body = "$body";
    } else if (m.file) body = `file_get_contents(${php(m.file.path)})`;
    return `<?php\n${setup.join("\n")}\n$ch = curl_init(${php(request.url)});\ncurl_setopt_array($ch, [\n  CURLOPT_CUSTOMREQUEST => ${php(m.method)},\n  CURLOPT_TIMEOUT_MS => ${m.timeoutMs},\n  CURLOPT_HTTPHEADER => [${m.headers.map((row) => php(`${row.name}: ${row.value}`)).join(", ")}${m.multipart ? `${m.headers.length ? ", " : ""}'Content-Type: multipart/form-data; boundary=' . $boundary` : ""}],${body ? `\n  CURLOPT_POSTFIELDS => ${body},` : ""}\n  CURLOPT_RETURNTRANSFER => true,\n]);\n$response = curl_exec($ch);\nif ($response === false) { throw new RuntimeException(curl_error($ch)); }\necho curl_getinfo($ch, CURLINFO_RESPONSE_CODE), "\\n", $response;\ncurl_close($ch);`;
  }
  if (language === "ruby") {
    const setup: string[] = [];
    if (m.multipart) {
      setup.push(
        `request.set_form([${m.form.map((part) => ("value" in part ? `[${ruby(part.name)}, ${ruby(part.value)}]` : `[${ruby(part.name)}, File.open(${ruby(part.file.path)}, "rb"), { filename: ${ruby(part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin")}, content_type: ${ruby(part.file.contentType || "application/octet-stream")} }]`)).join(", ")}], "multipart/form-data")`,
      );
    } else if (m.file) setup.push(`request.body = File.binread(${ruby(m.file.path)})`);
    else if (m.body) setup.push(`request.body = ${ruby(m.body)}`);
    return `require "net/http"\nuri = URI(${ruby(request.url)})\nrequest = Net::HTTPGenericRequest.new(${ruby(m.method)}, ${m.body || m.multipart || m.file ? "true" : "false"}, ${m.method === "HEAD" ? "false" : "true"}, uri.request_uri)\n${m.headers.map((row) => `request[${ruby(row.name)}] = ${ruby(row.value)}`).join("\n")}\n${setup.join("\n")}\nhttp = Net::HTTP.new(uri.hostname, uri.port)\nhttp.use_ssl = uri.scheme == "https"\nhttp.open_timeout = ${m.seconds}\nhttp.read_timeout = ${m.seconds}\nhttp.write_timeout = ${m.seconds}\nresponse = http.request(request)\nputs response.code, response.body`;
  }
  const setup: string[] = [];
  if (m.multipart) {
    setup.push("var body = new MultipartFormDataContent();");
    m.form.forEach((part, index) => {
      if ("value" in part) setup.push(`body.Add(new StringContent(${csharp(part.value)}, Encoding.UTF8), ${csharp(part.name)});`);
      else
        setup.push(
          `var file${index} = new StreamContent(File.OpenRead(${csharp(part.file.path)}));`,
          `file${index}.Headers.ContentType = new MediaTypeHeaderValue(${csharp(part.file.contentType || "application/octet-stream")});`,
          `body.Add(file${index}, ${csharp(part.name)}, ${csharp(part.file.fileName || part.file.path.split(/[\\/]/).at(-1) || "upload.bin")});`,
        );
    });
    setup.push("request.Content = body;");
  } else if (m.file) setup.push(`request.Content = new StreamContent(File.OpenRead(${csharp(m.file.path)}));`);
  else if (m.body) setup.push(`request.Content = new ByteArrayContent(Encoding.UTF8.GetBytes(${csharp(m.body)}));`);
  for (const row of m.headers) {
    if (row.name.toLowerCase().startsWith("content-") && (m.body || m.file)) setup.push(`request.Content!.Headers.TryAddWithoutValidation(${csharp(row.name)}, ${csharp(row.value)});`);
    else setup.push(`request.Headers.TryAddWithoutValidation(${csharp(row.name)}, ${csharp(row.value)});`);
  }
  return `using System;\nusing System.IO;\nusing System.Net.Http;\nusing System.Net.Http.Headers;\nusing System.Text;\n\nusing var client = new HttpClient { Timeout = TimeSpan.FromMilliseconds(${m.timeoutMs}) };\nusing var request = new HttpRequestMessage(new HttpMethod(${csharp(m.method)}), ${csharp(request.url)});\n${setup.join("\n")}\nusing var response = await client.SendAsync(request);\nConsole.WriteLine((int)response.StatusCode);\nConsole.WriteLine(await response.Content.ReadAsStringAsync());`;
}
