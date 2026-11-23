import type { OpenApiEnvironment, OpenApiEnvironmentInput, OpenApiEnvironmentServer } from "@/types/api";

export type EnvironmentVariableDraft = OpenApiEnvironmentInput["variables"][number] & { hasValue?: boolean; localValue?: string };
export type EnvironmentDraft = {
  id: string;
  name: string;
  slug: string;
  baseUrl: string;
  requestMode: OpenApiEnvironmentServer["requestMode"];
  servers?: OpenApiEnvironmentServer[];
  variables: EnvironmentVariableDraft[];
};

export function environmentDraft(environment: OpenApiEnvironment, local: Record<string, string>): EnvironmentDraft {
  return {
    ...environment,
    requestMode: environment.requestMode ?? "inherit",
    servers: environment.servers?.map((server) => ({ ...server })) ?? [],
    variables: environment.variables.map((row) => ({ ...row, localValue: Object.hasOwn(local, row.key) ? local[row.key] : undefined })),
  };
}

export function environmentExport(draft: EnvironmentDraft) {
  return {
    name: draft.name,
    slug: draft.slug,
    baseUrl: draft.baseUrl,
    requestMode: draft.requestMode,
    servers: draft.servers ?? [],
    variables: draft.variables.map(({ key, value, localValue, enabled, sensitive }) => ({
      key,
      enabled,
      sensitive,
      ...(sensitive ? {} : { value: localValue ?? value ?? "" }),
    })),
  };
}

export function environmentInput(draft: EnvironmentDraft): OpenApiEnvironmentInput {
  return {
    name: draft.name.trim(),
    baseUrl: draft.baseUrl.trim(),
    requestMode: draft.requestMode,
    servers: (draft.servers ?? []).map((server) => ({ ...server, name: server.name.trim(), baseUrl: server.baseUrl.trim() })),
    variables: draft.variables.map(({ key, enabled, sensitive, value }) => ({ key: key.trim(), enabled, sensitive, value: sensitive ? undefined : value })),
  };
}

export function prefixOpenApiRequestPath(base: string, path: string) {
  const target = path.trim();
  if (!base || /^[a-z][a-z\d+.-]*:/i.test(target)) return target;
  const prefix = base.replace(/\/+$/, "");
  if (!target || /^[?#]/.test(target)) return `${prefix}${target}`;
  return `${prefix}/${target.replace(/^\/+/, "")}`;
}
