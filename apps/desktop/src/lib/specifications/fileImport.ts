import { parseOpenApiContent } from "@/lib/openapi";
import type { SpecificationKind } from "@/types/domain";

export type ImportedSpecificationFile = { fileName: string; title: string; content: string; format?: "json" | "yaml" };

function readFileText(file: File): Promise<string> {
  if (typeof file.text === "function") return file.text();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => (typeof reader.result === "string" ? resolve(reader.result) : reject(new Error("read_failed")));
    reader.onerror = () => reject(new Error("read_failed"));
    reader.readAsText(file);
  });
}

export async function readSpecificationFile(file: File, kind: SpecificationKind): Promise<ImportedSpecificationFile> {
  const name = file.name.toLowerCase();
  const markdown = name.endsWith(".md") || name.endsWith(".markdown");
  const json = name.endsWith(".json");
  const yaml = name.endsWith(".yaml") || name.endsWith(".yml");
  if (kind === "markdown" ? !markdown : !json && !yaml) throw new Error("unsupported_type");
  if (file.size > 5 * 1024 * 1024) throw new Error("too_large");

  const content = await readFileText(file);
  const fallbackTitle = file.name.replace(/\.(markdown|md|yaml|yml|json)$/i, "");
  if (kind === "markdown") return { fileName: file.name, title: fallbackTitle, content };

  const document = parseOpenApiContent(content);
  if (!document || typeof document.openapi !== "string" || !(document.openapi.startsWith("3.0.") || document.openapi.startsWith("3.1."))) {
    throw new Error("invalid_openapi");
  }
  const info = document.info;
  const title = info && typeof info === "object" && !Array.isArray(info) && "title" in info && typeof info.title === "string" ? info.title.trim() : "";
  return { fileName: file.name, title: title || fallbackTitle, content, format: json ? "json" : "yaml" };
}
