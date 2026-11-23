/** Route placeholders are distinct from {{environment}} substitutions. */
const routeParameter = /(?<!\{)\{([^{}/?#]+)\}(?!\})/gu;
export function extractPathParameterNames(url: string): string[] {
  return [...new Set([...url.split(/[?#]/u, 1)[0]!.matchAll(routeParameter)].map((match) => match[1]!))];
}
export function resolvePathTemplate(url: string, values: Record<string, string>, encodeValue: (value: string) => string = encodeURIComponent): string {
  const end = url.search(/[?#]/u);
  const path = end < 0 ? url : url.slice(0, end);
  return path.replace(routeParameter, (placeholder: string, name: string) => (Object.hasOwn(values, name) ? encodeValue(values[name]!) : placeholder)) + (end < 0 ? "" : url.slice(end));
}

/** Restores the documented route shape for a concrete request URL when its
 * path still matches the operation. The original host, base path and query are
 * kept so an edited target remains editable after loading history or a case. */
export function restorePathTemplate(url: string, routeTemplate: string): string {
  if (!routeTemplate || extractPathParameterNames(url).length) return url;
  const hashOrQuery = url.search(/[?#]/u);
  const targetPath = hashOrQuery < 0 ? url : url.slice(0, hashOrQuery);
  const suffix = hashOrQuery < 0 ? "" : url.slice(hashOrQuery);
  const routePath = routeTemplate.split(/[?#]/u, 1)[0]!;
  const targetSegments = targetPath.split("/");
  const routeSegments = routePath.replace(/^\//u, "").split("/");
  const offset = targetSegments.length - routeSegments.length;
  if (offset < 0) return url;
  const decoded = (value: string) => {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  };
  for (let index = 0; index < routeSegments.length; index++) {
    const routeSegment = routeSegments[index]!;
    const targetSegment = targetSegments[offset + index]!;
    if (!routeSegment.includes("{")) {
      if (decoded(targetSegment) !== decoded(routeSegment)) return url;
      continue;
    }
    const match = /^([^{}]*)\{[^{}]+\}([^{}]*)$/u.exec(routeSegment);
    if (!match || !targetSegment.startsWith(match[1]!) || !targetSegment.endsWith(match[2]!)) return url;
  }
  for (let index = 0; index < routeSegments.length; index++) targetSegments[offset + index] = routeSegments[index]!;
  return `${targetSegments.join("/")}${suffix}`;
}
