import type { OpenApiObject } from "../contract/openapi";

export const responseFormatPresets = [
  ["application/json", "JSON"],
  ["application/xml", "XML"],
  ["text/html", "HTML"],
  ["text/plain", "Raw"],
  ["application/octet-stream", "Binary"],
  ["application/msgpack", "MsgPack"],
  ["text/event-stream", "Event Stream"],
  ["application/yaml", "YAML"],
  ["application/x-www-form-urlencoded", "Form URL Encoded"],
  ["multipart/form-data", "Multipart Form"],
  ["application/pdf", "PDF"],
] as const;

export function responseFormatLabel(mediaType: string): string {
  return responseFormatPresets.find(([type]) => type === mediaType.toLowerCase())?.[1] ?? mediaType;
}

export function responseFormatKind(mediaType: string): "schema" | "text" | "binary" {
  const type = mediaType.toLowerCase();
  if (/\/(?:json|[^/]+\+json)$/.test(type) || ["application/x-www-form-urlencoded", "multipart/form-data", "application/msgpack", "application/x-msgpack"].includes(type)) return "schema";
  if (/^(?:text\/|application\/(?:xml$|[^/]+\+xml$|(?:x-)?yaml$|javascript$))/.test(type)) return "text";
  return "binary";
}

export function createResponseMediaSchema(mediaType: string): OpenApiObject {
  const kind = responseFormatKind(mediaType);
  if (kind === "schema") return { type: "object", properties: {} };
  return kind === "binary" ? { type: "string", format: "binary" } : { type: "string" };
}
