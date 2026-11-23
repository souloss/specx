import type { SpecificationKind } from "@/types/domain";
import { specificationType } from "./registry";

export type ImportedSpecificationFile = { fileName: string; title: string; content: string; format?: "json" | "yaml" };

export function specificationFileExtension(name: string, kind: SpecificationKind) {
  const normalized = name.toLowerCase();
  return specificationType(kind).file.extensions.find((extension) => normalized.endsWith(extension));
}

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
  const extension = specificationFileExtension(file.name, kind);
  if (!extension) throw new Error("unsupported_type");
  if (file.size > 5 * 1024 * 1024) throw new Error("too_large");

  const content = await readFileText(file);
  const inspected = specificationType(kind).file.inspect?.(content, extension);
  return { fileName: file.name, content, ...inspected, title: inspected?.title || file.name.slice(0, -extension.length) };
}
