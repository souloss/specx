import { isRecord, type OpenApiObject } from "../contract/openapi";
import type { OpenApiEnvironment, PublicOpenApiEnvironment } from "@/types/api";

export const publicEnvironmentExtension = "x-specx-public-environments";

export function publicEnvironmentVariables(environment: PublicOpenApiEnvironment): Record<string, string> {
  return Object.fromEntries(environment.variables.map(({ key, value }) => [key, value]));
}
export function expandPublicVariables(value: string, variables: Record<string, string>): string {
  return value.replace(/\{\{([^}]+)\}\}|<<([^>]+)>>/g, (match, moustache: string | undefined, angle: string | undefined) => variables[(moustache ?? angle ?? "").trim()] ?? match);
}
export function publicEnvironmentUrl(environment: PublicOpenApiEnvironment): string {
  return expandPublicVariables(environment.baseUrl, publicEnvironmentVariables(environment));
}

export function validPublicEnvironments(environments: PublicOpenApiEnvironment[]): boolean {
  if (environments.length > 20) return false;
  const names = new Set<string>();
  const bytes = (value: string) => new TextEncoder().encode(value).length;
  for (const environment of environments) {
    const name = environment.name.trim().toLowerCase();
    if (!name || bytes(environment.name) > 80 || /\p{Cc}/u.test(environment.name) || names.has(name) || bytes(environment.baseUrl) > 2000 || environment.variables.length > 100) return false;
    names.add(name);
    const keys = new Set<string>();
    for (const variable of environment.variables) {
      if (!/^[\w.-]{1,120}$/.test(variable.key) || keys.has(variable.key) || bytes(variable.value) > 65536 || /\p{Cc}/u.test(variable.value)) return false;
      keys.add(variable.key);
    }
    const url = publicEnvironmentUrl(environment);
    if (!/^https?:\/\//.test(url) || /[\s\\{}<>#]/u.test(url)) return false;
    try {
      const parsed = new URL(url);
      if (!parsed.hostname || parsed.username || parsed.password || parsed.search) return false;
    } catch {
      return false;
    }
  }
  return true;
}

export function readPublicEnvironments(document: OpenApiObject | null): PublicOpenApiEnvironment[] {
  const value = document?.[publicEnvironmentExtension];
  if (
    !Array.isArray(value) ||
    !value.every((item) => isRecord(item) && typeof item.name === "string" && typeof item.baseUrl === "string" && Array.isArray(item.variables) && item.variables.every((variable) => isRecord(variable) && typeof variable.key === "string" && typeof variable.value === "string"))
  )
    return [];
  const environments = value as PublicOpenApiEnvironment[];
  return validPublicEnvironments(environments) ? environments : [];
}

export function copyPublicEnvironment(source: OpenApiEnvironment): PublicOpenApiEnvironment {
  return { name: source.name, baseUrl: source.baseUrl, variables: source.variables.filter((item) => item.enabled && !item.sensitive && item.value !== null).map((item) => ({ key: item.key, value: item.value! })) };
}

export function withPublicEnvironments(content: string, environments: PublicOpenApiEnvironment[]): string {
  if (!environments.length || !validPublicEnvironments(environments)) return content;
  const document = JSON.parse(content) as OpenApiObject;
  document[publicEnvironmentExtension] = environments;
  document.servers = environments.map((item) => ({ url: publicEnvironmentUrl(item), description: item.name }));
  return JSON.stringify(document);
}
