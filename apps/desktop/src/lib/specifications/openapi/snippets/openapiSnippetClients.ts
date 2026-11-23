/** One registry for documentation examples and the request workbench. */
export const SNIPPET_CLIENTS = [
  { id: "curl", family: "Shell", label: "cURL", extension: "sh" },
  { id: "curlWindows", family: "Shell", label: "cURL · PowerShell", extension: "ps1" },
  { id: "httpie", family: "Shell", label: "HTTPie", extension: "sh" },
  { id: "wget", family: "Shell", label: "wget", extension: "sh" },
  { id: "powershell", family: "Shell", label: "PowerShell · HttpClient", extension: "ps1" },
  { id: "fetch", family: "JavaScript", label: "Fetch", extension: "mjs" },
  { id: "axios", family: "JavaScript", label: "Axios", extension: "mjs" },
  { id: "jquery", family: "JavaScript", label: "jQuery", extension: "js" },
  { id: "xhr", family: "JavaScript", label: "XHR", extension: "js" },
  { id: "node", family: "JavaScript", label: "Node.js · Native", extension: "mjs" },
  { id: "request", family: "JavaScript", label: "Request", extension: "mjs" },
  { id: "unirest", family: "JavaScript", label: "Unirest", extension: "mjs" },
  { id: "java", family: "Java", label: "HttpClient", extension: "java" },
  { id: "javaUnirest", family: "Java", label: "Unirest", extension: "java" },
  { id: "okhttp", family: "Java", label: "OkHttp", extension: "java" },
  { id: "python", family: "Python", label: "requests", extension: "py" },
  { id: "go", family: "Go", label: "net/http", extension: "go" },
  { id: "php", family: "PHP", label: "cURL", extension: "php" },
  { id: "ruby", family: "Ruby", label: "Net::HTTP", extension: "rb" },
  { id: "csharp", family: "C#", label: "HttpClient", extension: "cs" },
] as const;
export type SnippetLanguage = (typeof SNIPPET_CLIENTS)[number]["id"];
export const SNIPPET_FAMILIES = [...new Set(SNIPPET_CLIENTS.map((client) => client.family))];
export function snippetFileName(language: SnippetLanguage): string {
  const client = SNIPPET_CLIENTS.find((client) => client.id === language)!;
  return client.extension === "java" ? "SpecXRequest.java" : `request.${client.extension}`;
}
