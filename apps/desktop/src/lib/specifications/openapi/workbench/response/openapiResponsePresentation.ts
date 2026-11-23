import iconv from "iconv-lite";
import { Buffer } from "buffer";
export const responseEncodings = ["utf8", "ISO-8859-1", "ISO-8859-16", "GB2312", "GBK", "ascii", "binary", "base64", "hex", "utf16", "utf16-be", "utf16-le", "utf-7", "Shift_JIS"] as const;
export type ResponseEncoding = (typeof responseEncodings)[number];
export function responseEncodingForContentType(contentType: string): ResponseEncoding {
  const charset = /charset\s*=\s*["']?([^;\s"']+)/iu.exec(contentType)?.[1]?.toLowerCase().replaceAll("_", "-");
  const aliases: Record<string, ResponseEncoding> = { "utf-8": "utf8", "utf-16": "utf16", "utf-16le": "utf16-le", "utf-16be": "utf16-be", "us-ascii": "ascii", latin1: "ISO-8859-1", "shift-jis": "Shift_JIS", sjis: "Shift_JIS" };
  return charset ? (aliases[charset] ?? responseEncodings.find((item) => item.toLowerCase().replaceAll("_", "-") === charset) ?? "utf8") : "utf8";
}
export const responseFormats = ["auto", "json", "xml", "html", "text"] as const;
export type ResponseFormat = (typeof responseFormats)[number];
function textDecoder(bytes: Uint8Array, encoding: string): string {
  try {
    return new TextDecoder(encoding, { fatal: false }).decode(bytes);
  } catch {
    return Buffer.from(bytes).toString("latin1");
  }
}
export function decodeResponseBytes(bytes: Uint8Array, encoding: ResponseEncoding): string {
  if (encoding === "hex") return Buffer.from(bytes).toString("hex");
  if (encoding === "base64") return Buffer.from(bytes).toString("base64");
  if (encoding === "binary") return Buffer.from(bytes).toString("latin1");
  if (encoding === "ascii") return Buffer.from(bytes).toString("ascii");
  if (encoding === "GBK" || encoding === "GB2312") return iconv.decode(Buffer.from(bytes), "gbk");
  if (encoding === "ISO-8859-1" || encoding === "ISO-8859-16") return iconv.decode(Buffer.from(bytes), encoding);
  if (encoding === "utf16") return iconv.decode(Buffer.from(bytes), "utf16");
  if (encoding === "utf16-be") return textDecoder(bytes, "utf-16be");
  if (encoding === "utf16-le") return textDecoder(bytes, "utf-16le");
  if (encoding === "Shift_JIS") return iconv.decode(Buffer.from(bytes), "shift_jis");
  if (encoding === "utf-7") return iconv.decode(Buffer.from(bytes), "utf7");
  return textDecoder(bytes, "utf-8");
}
