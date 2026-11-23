import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import powershell from "highlight.js/lib/languages/powershell";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import json from "highlight.js/lib/languages/json";
import java from "highlight.js/lib/languages/java";
import python from "highlight.js/lib/languages/python";
import go from "highlight.js/lib/languages/go";
import php from "highlight.js/lib/languages/php";
import ruby from "highlight.js/lib/languages/ruby";
import csharp from "highlight.js/lib/languages/csharp";

for (const [name, grammar] of Object.entries({ bash, powershell, javascript, typescript, json, java, python, go, php, ruby, csharp })) hljs.registerLanguage(name, grammar);

const aliases: Record<string, string> = {
  curl: "bash",
  curlWindows: "powershell",
  shell: "bash",
  httpie: "bash",
  wget: "bash",
  fetch: "javascript",
  axios: "javascript",
  jquery: "javascript",
  xhr: "javascript",
  node: "javascript",
  request: "javascript",
  unirest: "javascript",
  javaUnirest: "java",
  okhttp: "java",
};

export function highlightOpenApiCode(text: string, language: string): string {
  const grammar = aliases[language] ?? (language.includes("json") ? "json" : language);
  // Highlight.js escapes source text before inserting its own span markup.
  if (text.length <= 100_000 && hljs.getLanguage(grammar)) return hljs.highlight(text, { language: grammar, ignoreIllegals: true }).value;
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;").replaceAll("'", "&#39;");
}
