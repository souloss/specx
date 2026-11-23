import { expandOpenApiServerUrl, formatOpenApiExample, generateOpenApiExample, getOpenApiMediaExamples, getOpenApiServers, isRecord, mergeOpenApiSchema, resolveOpenApiRef, type OpenApiObject, type OpenApiOperationEntry } from "../contract/openapi";
import { getOperationParameters, serializeOpenApiParameter } from "../contract/openapiParameters";
import { localOpenApiDefinitions } from "../contract/openapiReferences";
import type { WorkbenchRequest } from "../workbench/openapiWorkbench";

export type OpenApiRequestExampleSelection = { mediaType: string; exampleName: string };

/** Build illustrative code from the definition without resolving workspace data. */
export function operationExampleRequest(document: OpenApiObject | null, entry: OpenApiOperationEntry, selection?: OpenApiRequestExampleSelection): WorkbenchRequest {
  const server = getOpenApiServers(document, entry)[0]?.url ?? "https://api.example.com";
  let url = `${expandOpenApiServerUrl(document, server, {}, entry).replace(/\/$/, "")}${entry.path}`;
  const headers: WorkbenchRequest["headers"] = [];
  const cookies: WorkbenchRequest["cookies"] = [];
  const queries: Array<[string, string]> = [];
  for (const parameter of getOperationParameters(document, entry)) {
    const schema = mergeOpenApiSchema(document, parameter.schema);
    const examples = isRecord(parameter.examples)
      ? Object.values(parameter.examples)
          .map((example) => resolveOpenApiRef(document, example))
          .find((example) => example && Object.hasOwn(example, "value"))
      : undefined;
    const value = parameter.example ?? examples?.value ?? generateOpenApiExample(document, parameter.schema);
    const raw = typeof value === "object" ? JSON.stringify(value) : String(value ?? "");
    const serialized = serializeOpenApiParameter(parameter, raw, schema);
    if (!serialized.ok) continue;
    const name = String(parameter.name);
    if (parameter.in === "path") url = url.replace(`{${name}}`, serialized.value.pathValue);
    if (parameter.in === "query") queries.push(...serialized.value.queryEntries);
    if (parameter.in === "header") headers.push({ name, value: serialized.value.headerValue, enabled: true });
    if (parameter.in === "cookie") cookies.push(...serialized.value.cookieEntries.map(([name, value]) => ({ name, value, enabled: true })));
  }
  // Never embed a workspace credential in generated documentation. The first
  // non-anonymous alternative is illustrative; the full choices remain in docs.
  const requirements = entry.operation.security ?? document?.security;
  const requirement = Array.isArray(requirements) && !requirements.some((item) => isRecord(item) && !Object.keys(item).length) ? requirements.find(isRecord) : undefined;
  const components = isRecord(document?.components) ? document.components : undefined;
  const schemes = isRecord(components?.securitySchemes) ? components.securitySchemes : {};
  for (const name of Object.keys(requirement ?? {})) {
    const scheme = resolveOpenApiRef(document, schemes[name]);
    if (scheme?.type === "apiKey" && typeof scheme.name === "string") {
      if (scheme.in === "query") queries.push([encodeURIComponent(scheme.name), "YOUR_API_KEY"]);
      if (scheme.in === "header") headers.push({ name: scheme.name, value: "YOUR_API_KEY", enabled: true });
      if (scheme.in === "cookie") cookies.push({ name: scheme.name, value: "YOUR_API_KEY", enabled: true });
    } else if ((scheme?.type === "http" && ["basic", "bearer"].includes(String(scheme.scheme))) || scheme?.type === "oauth2" || scheme?.type === "openIdConnect") {
      headers.push({ name: "Authorization", value: scheme.scheme === "basic" ? "Basic BASE64_USERNAME_PASSWORD" : "Bearer YOUR_ACCESS_TOKEN", enabled: true });
    }
  }
  if (queries.length) url += `${url.includes("?") ? "&" : "?"}${queries.map(([name, value]) => `${name}=${value}`).join("&")}`;
  const requestBody = resolveOpenApiRef(document, entry.operation.requestBody);
  const mediaEntries = isRecord(requestBody?.content) ? Object.entries(requestBody.content).filter(([, value]) => isRecord(value)) : [];
  const media = mediaEntries.find(([type]) => type === selection?.mediaType) ?? mediaEntries[0];
  const mediaType = media?.[0] ?? "";
  const examples = media ? getOpenApiMediaExamples(document, media[1]) : [];
  const example = examples.find((item) => item.name === selection?.exampleName) ?? examples[0];
  const value = media ? (example ? example.value : generateOpenApiExample(document, isRecord(media[1]) ? media[1].schema : undefined)) : undefined;
  const form = ["application/x-www-form-urlencoded", "multipart/form-data"].includes(mediaType);
  const body =
    form && isRecord(value)
      ? new URLSearchParams(Object.entries(value).flatMap(([name, item]) => (Array.isArray(item) ? item : [item]).map((part) => [name, typeof part === "object" ? JSON.stringify(part) : String(part ?? "")]))).toString()
      : value === undefined
        ? ""
        : formatOpenApiExample(value, mediaType);
  return { method: entry.method.toUpperCase(), url, headers, cookies, mediaType, body };
}

/** Copy/export one page as Markdown while retaining all referenced definitions. */
export function operationMarkdown(document: OpenApiObject | null, entry: OpenApiOperationEntry): string {
  const title = String(entry.operation.summary || entry.operation.operationId || `${entry.method.toUpperCase()} ${entry.path}`);
  const parameters = getOperationParameters(document, entry);
  const sections = [`# ${title}`, `\`${entry.method.toUpperCase()} ${entry.path}\``, String(entry.operation.description ?? "")];
  const security = entry.operation.security ?? document?.security;
  const schemes = isRecord(document?.components) && isRecord(document.components.securitySchemes) ? document.components.securitySchemes : {};
  const usedSchemes: OpenApiObject = {};
  if (Array.isArray(security)) {
    for (const requirement of security.filter(isRecord)) {
      for (const name of Object.keys(requirement)) {
        const scheme = resolveOpenApiRef(document, schemes[name]);
        if (scheme) usedSchemes[name] = scheme;
      }
    }
    sections.push("## Authentication", "```json\n" + JSON.stringify({ security, securitySchemes: usedSchemes }, null, 2) + "\n```");
  }
  if (parameters.length) sections.push("## Parameters", "```json\n" + JSON.stringify(parameters, null, 2) + "\n```");
  if (entry.operation.requestBody) sections.push("## Request body", "```json\n" + JSON.stringify(resolveOpenApiRef(document, entry.operation.requestBody), null, 2) + "\n```");
  sections.push("## Responses", "```json\n" + JSON.stringify(entry.operation.responses ?? {}, null, 2) + "\n```");
  // Retain only the local reference closure, including cycles, for a portable
  // operation page without copying every unrelated schema in a large API.
  const definitions = localOpenApiDefinitions(document, { parameters, securitySchemes: usedSchemes, requestBody: entry.operation.requestBody, responses: entry.operation.responses });
  if (Object.keys(definitions).length) sections.push("## Referenced definitions", "```json\n" + JSON.stringify(definitions, null, 2) + "\n```");
  return sections.filter(Boolean).join("\n\n");
}
