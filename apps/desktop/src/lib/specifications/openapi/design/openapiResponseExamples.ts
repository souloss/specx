import { json } from "@codemirror/lang-json";
import { formatOpenApiExample, isRecord, type OpenApiObject } from "../contract/openapi";

export type ResponseExampleTarget = { status: string; mediaType: string };
export type ResponseExampleInput = { name: string; body: string };
export type ResponseCaptureTarget = ResponseExampleTarget & { action: "newResponse" | "newMedia" | "existing"; canInfer: boolean; names: string[] };
export type CapturedResponseExample = ResponseExampleInput & Pick<ResponseCaptureTarget, "status" | "mediaType" | "action"> & { operationKey: string; infer: boolean };
export type ResponseExampleEdits = Record<string, Record<string, string | null>>;

const MAX_EXAMPLE_BYTES = 32 * 1024;
const jsonMedia = (mediaType: string) => /\/(?:json|[^/]+\+json)$/i.test(mediaType);

export function supportsResponseExample(mediaType: string): boolean {
  return jsonMedia(mediaType) || /^(?:text\/|application\/(?:xml$|[^/]+\+xml$|(?:x-)?yaml$|javascript$|x-www-form-urlencoded$))/i.test(mediaType);
}

export function responseHasBody(status: string): boolean {
  return !status.startsWith("1") && !["204", "205", "304"].includes(status);
}

/** Capture always targets the actual status, even when a range/default handles validation. */
export function responseCaptureTarget(definitions: unknown, status: string, contentType: string): ResponseCaptureTarget | null {
  const mediaType = contentType.split(";", 1)[0]?.trim().toLowerCase() ?? "";
  if (!/^[2-5]\d{2}$/.test(status) || !responseHasBody(status) || !/^[\w!#$%&'+.^`|~-]+\/[\w!#$%&'+.^`|~-]+$/.test(mediaType) || !supportsResponseExample(mediaType)) return null;
  if (definitions === undefined || (isRecord(definitions) && !Object.hasOwn(definitions, status))) return { status, mediaType, action: "newResponse", canInfer: true, names: [] };
  if (!isRecord(definitions)) return null;
  const response = definitions[status];
  if (!isRecord(response) || "$ref" in response || (response.content !== undefined && !isRecord(response.content))) return null;
  const content = isRecord(response.content) ? response.content : {};
  const matching = Object.keys(content).filter((key) => key.toLowerCase() === mediaType);
  if (!matching.length) return { status, mediaType, action: "newMedia", canInfer: true, names: [] };
  const key = matching[0]!;
  const media = content[key];
  if (matching.length > 1 || !isRecord(media) || "$ref" in media) return null;
  return { status, mediaType: key, action: "existing", canInfer: !Object.hasOwn(media, "schema"), names: responseExampleRows(media, key).map((example) => example.name) };
}

export function validResponseExampleName(name: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name);
}

function decimalIdentity(source: string): string {
  const [mantissa = "", power = "0"] = source.toLowerCase().split("e");
  const [whole = "", fraction = ""] = mantissa.replace(/^-/, "").split(".");
  const digits = (whole + fraction).replace(/^0+/, "");
  if (!digits) return mantissa.startsWith("-") ? "-0" : "0";
  const significant = digits.replace(/0+$/, "");
  return `${mantissa.startsWith("-") ? "-" : ""}${significant}e${Number(power) - fraction.length + digits.length - significant.length}`;
}

export function parseResponseExample(body: string, mediaType: string): { value: unknown } | { error: "tooLarge" | "invalidJson" | "lossyJson" } {
  if (body.length > MAX_EXAMPLE_BYTES || new TextEncoder().encode(body).length > MAX_EXAMPLE_BYTES) return { error: "tooLarge" };
  if (!jsonMedia(mediaType)) return { value: body };
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    return { error: "invalidJson" };
  }
  const cursor = json().language.parser.parse(body).cursor();
  const keys: Set<string>[] = [];
  let depth = 0;
  try {
    cursor.iterate(
      (node) => {
        if (node.name === "Array" || node.name === "Object") depth += 1;
        if (depth > 64) throw new RangeError("example_depth");
        if (node.name === "Object") keys.push(new Set());
        if (node.name === "PropertyName") {
          const key: string = JSON.parse(body.slice(node.from, node.to));
          const scope = keys.at(-1)!;
          if (scope.has(key)) throw new RangeError("duplicate_key");
          scope.add(key);
        } else if (node.name === "Number") {
          const literal = body.slice(node.from, node.to);
          const number = Number(literal);
          if (!Number.isFinite(number) || decimalIdentity(literal) !== decimalIdentity(String(number))) throw new RangeError("example_number");
        }
      },
      (node) => {
        if (node.name === "Array" || node.name === "Object") depth -= 1;
        if (node.name === "Object") keys.pop();
      },
    );
  } catch {
    return { error: "lossyJson" };
  }
  return { value };
}

export function responseExampleRows(media: OpenApiObject | undefined, mediaType: string, edits: Record<string, string | null> = {}) {
  const examples = isRecord(media?.examples) ? media.examples : {};
  const rows = Object.entries(examples).map(([name, example]) => ({
    name,
    body: isRecord(example) && !("$ref" in example) && Object.hasOwn(example, "value") ? formatOpenApiExample(example.value, mediaType) : null,
  }));
  if (media && Object.hasOwn(media, "example")) rows.unshift({ name: "", body: formatOpenApiExample(media.example, mediaType) });
  return [...rows.filter((row) => !Object.hasOwn(edits, row.name)), ...Object.entries(edits).flatMap(([name, body]) => (body === null ? [] : [{ name, body }]))];
}

export function applyResponseExampleEdits(media: OpenApiObject, mediaType: string, edits: Record<string, string | null>): OpenApiObject {
  const next = { ...media };
  const examples = isRecord(media.examples) ? { ...media.examples } : {};
  for (const [name, body] of Object.entries(edits)) {
    if (body === null) {
      if (name) delete examples[name];
      else delete next.example;
      continue;
    }
    const parsed = parseResponseExample(body, mediaType);
    if ("error" in parsed) continue;
    if (!name) next.example = parsed.value;
    else Object.defineProperty(examples, name, { value: { ...(Object.hasOwn(examples, name) && isRecord(examples[name]) ? examples[name] : {}), value: parsed.value }, enumerable: true, configurable: true, writable: true });
  }
  if (Object.keys(examples).length) {
    if (Object.hasOwn(next, "example")) {
      let defaultName = "default-example";
      for (let suffix = 2; Object.hasOwn(examples, defaultName); suffix += 1) defaultName = `default-example-${suffix}`;
      Object.defineProperty(examples, defaultName, { value: { value: next.example }, enumerable: true, configurable: true, writable: true });
      delete next.example;
    }
    next.examples = examples;
  } else delete next.examples;
  return next;
}
