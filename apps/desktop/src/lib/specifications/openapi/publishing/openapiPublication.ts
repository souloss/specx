type LocationContext = Pick<Location, "hostname" | "port" | "protocol" | "pathname">;

const PUBLICATION_ID = /^[a-f0-9]{32}$/;

function isIpAddress(hostname: string) {
  return hostname.includes(":") || /^\d{1,3}(?:\.\d{1,3}){3}$/.test(hostname);
}

export function openApiPublicationUrl(id: string, location: LocationContext = window.location) {
  if (!PUBLICATION_ID.test(id)) throw new Error("Invalid OpenAPI publication id");
  const baseHost = isIpAddress(location.hostname) ? "localhost" : location.hostname;
  return `${location.protocol}//${id}.${baseHost}${location.port ? `:${location.port}` : ""}/`;
}

export function publicOpenApiPublicationId(location: LocationContext = window.location) {
  const pathMatch = /^\/docs\/openapi\/([a-f0-9]{32})\/?$/.exec(location.pathname);
  if (pathMatch?.[1]) return pathMatch[1];
  const hostMatch = /^([a-f0-9]{32})\./.exec(location.hostname);
  return hostMatch?.[1];
}
