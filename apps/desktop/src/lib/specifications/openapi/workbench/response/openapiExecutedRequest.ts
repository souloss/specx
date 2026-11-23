export type ExecutedRequestSnapshot = { method: string; url: string; headers: Array<[string, string]>; mediaType: string; body: string; bodyBytes: number; binary: boolean };

/** Decode the actual upload, including final-hop method/body rewrites. */
export function describeExecutedRequest(input: { method: string; url: string; headers: Array<[string, string]>; body: Uint8Array }): ExecutedRequestSnapshot {
  const mediaType = input.headers.find(([name]) => name.toLowerCase() === "content-type")?.[1] ?? "";
  const binary = input.body.length > 0 && Boolean(mediaType) && !/^(?:text\/|multipart\/)|json|xml|yaml|javascript|x-www-form-urlencoded/iu.test(mediaType);
  let body: string;
  if (binary) body = Array.from({ length: Math.ceil(input.body.length / 16) }, (_, line) => [...input.body.subarray(line * 16, (line + 1) * 16)].map((byte) => byte.toString(16).padStart(2, "0")).join(" ")).join("\n");
  else {
    let decoder: TextDecoder;
    try {
      decoder = new TextDecoder(/charset\s*=\s*["']?([^\s;"']+)/iu.exec(mediaType)?.[1] ?? "utf-8");
    } catch {
      decoder = new TextDecoder();
    }
    body = decoder.decode(input.body);
  }
  return { method: input.method, url: input.url, headers: input.headers, mediaType, body, bodyBytes: input.body.length, binary };
}
