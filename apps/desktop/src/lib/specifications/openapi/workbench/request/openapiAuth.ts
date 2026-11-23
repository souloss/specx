import type { WorkbenchRequest } from "../openapiWorkbench";

export type OpenApiAuthInput = { value: string; username: string; password: string };
export type OpenApiAuthConfig = { mode: "specification" | "none" | "scheme"; alternative: number; scheme?: string; inputs: Record<string, OpenApiAuthInput> };
export type OpenApiAuthScheme = { name: string; scheme: Record<string, unknown> };
export type OpenApiAuthIssue = { code: "missing" | "unsupported" | "conflict" | "invalid"; scheme: string };
export type OpenApiAuthTargets = { headers: string[]; query: string[]; cookies: string[] };
export type OpenApiAuthResult = { url: string; headers: Headers; targets: OpenApiAuthTargets; issues: OpenApiAuthIssue[] };
export const emptyAuthInput = (): OpenApiAuthInput => ({ value: "", username: "", password: "" });
export const defaultAuthConfig = (mode: OpenApiAuthConfig["mode"] = "specification"): OpenApiAuthConfig => ({ mode, alternative: 0, inputs: {} });
const TEMPLATE = /^\{\{\s*([A-Za-z_][A-Za-z0-9_.-]*)\s*\}\}$/;

export function isAuthTemplate(value: string): boolean {
  return TEMPLATE.test(value.trim()) && TEMPLATE.exec(value.trim())?.[1]?.toLowerCase() !== "redacted";
}
export function sanitizeAuthConfig(config: OpenApiAuthConfig): OpenApiAuthConfig {
  const safe = (value: string) => (!value || isAuthTemplate(value) ? value : "{{redacted}}");
  return { ...config, inputs: Object.fromEntries(Object.entries(config.inputs).map(([name, input]) => [name, { value: safe(input.value), username: safe(input.username), password: safe(input.password) }])) };
}
export function isAuthConfig(value: unknown): value is OpenApiAuthConfig {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  const validName = (name: unknown) => typeof name === "string" && name.length > 0 && new TextEncoder().encode(name).length <= 256 && !/\p{Cc}/u.test(name);
  if (!["specification", "none", "scheme"].includes(String(v.mode)) || !Number.isInteger(v.alternative) || Number(v.alternative) < 0 || Number(v.alternative) >= 64 || (v.scheme !== undefined && !validName(v.scheme)) || (v.mode === "scheme" && !validName(v.scheme))) return false;
  if (!v.inputs || typeof v.inputs !== "object" || Array.isArray(v.inputs)) return false;
  const inputs = Object.entries(v.inputs);
  return (
    inputs.length <= 64 &&
    inputs.every(([name, input]: [string, unknown]) => {
      if (!validName(name) || !input || typeof input !== "object" || Array.isArray(input)) return false;
      const fields = input as Record<string, unknown>;
      return Object.keys(fields).every((key) => ["value", "username", "password"].includes(key)) && ["value", "username", "password"].every((key) => typeof fields[key] === "string" && new TextEncoder().encode(fields[key]).length <= 16_384 && !/\p{Cc}/u.test(fields[key]));
    }) &&
    Object.keys(v).every((key) => ["mode", "alternative", "scheme", "inputs"].includes(key))
  );
}
export function authVariableNames(name: string, field: keyof OpenApiAuthInput, scheme: Record<string, unknown>): string[] {
  return field === "username" ? [`${name}_USERNAME`, "USERNAME", "username"] : field === "password" ? [`${name}_PASSWORD`, "PASSWORD", "password"] : scheme.type === "apiKey" ? [name, `${name}_KEY`, "API_KEY"] : [name, `${name}_TOKEN`, "TOKEN", "token"];
}
export function authSchemeSupported(scheme: Record<string, unknown>): boolean {
  return scheme.type === "apiKey"
    ? typeof scheme.name === "string" && Boolean(scheme.name) && !/\p{Cc}/u.test(scheme.name) && (scheme.in === "query" || /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(scheme.name)) && ["header", "query", "cookie"].includes(String(scheme.in))
    : scheme.type === "http"
      ? ["basic", "bearer"].includes(String(scheme.scheme).toLowerCase())
      : ["oauth2", "openIdConnect"].includes(String(scheme.type));
}
export function selectedAuthNames(config: OpenApiAuthConfig, requirements: Record<string, unknown>[]): string[] {
  return config.mode === "none" ? [] : config.mode === "scheme" ? (config.scheme ? [config.scheme] : []) : Object.keys(requirements[config.alternative] ?? {});
}
function variableValue(variables: Record<string, string>, names: string[]): string | undefined {
  for (const name of names) for (const key of [name, name.toUpperCase(), name.toLowerCase()]) if (Object.hasOwn(variables, key)) return variables[key];
  return undefined;
}
function inputValue(input: string | undefined, variables: Record<string, string>, aliases: string[]): string | undefined {
  if (!input || input === "{{redacted}}") return variableValue(variables, aliases);
  const match = TEMPLATE.exec(input.trim());
  return match ? variableValue(variables, [match[1]!]) : input.includes("{{") ? undefined : input;
}
function usable(value: string | undefined): value is string {
  return value !== undefined && Boolean(value) && !value.includes("{{") && !/[\r\n]/.test(value);
}
function replaceQuery(url: string, name: string, value: string): string {
  const hashIndex = url.indexOf("#");
  const hash = hashIndex < 0 ? "" : url.slice(hashIndex);
  const base = hashIndex < 0 ? url : url.slice(0, hashIndex);
  const queryIndex = base.indexOf("?");
  const path = queryIndex < 0 ? base : base.slice(0, queryIndex);
  const query = queryIndex < 0 ? "" : base.slice(queryIndex + 1);
  const retained = query
    .split("&")
    .filter(Boolean)
    .filter((pair) => {
      try {
        return decodeURIComponent(pair.split("=", 1)[0]!.replaceAll("+", " ")) !== name;
      } catch {
        return true;
      }
    });
  return `${path}?${[...retained, `${encodeURIComponent(name)}=${encodeURIComponent(value)}`].join("&")}${hash}`;
}

/** Apply one OR alternative, including every AND member, without mutating the source request. */
export function applyOpenApiAuth(options: {
  config: OpenApiAuthConfig;
  schemes: OpenApiAuthScheme[];
  requirements: Record<string, unknown>[];
  url: string;
  headers: Headers;
  variables?: Record<string, string>;
  tokens?: Record<string, string>;
  preview?: boolean;
  disabledHeaders?: string[];
}): OpenApiAuthResult {
  const { config, schemes, requirements, preview } = options;
  const variables = options.variables ?? {};
  const result: OpenApiAuthResult = { url: options.url, headers: new Headers(options.headers), targets: { headers: [], query: [], cookies: [] }, issues: [] };
  if (config.mode === "specification" && requirements.length && !requirements[config.alternative]) {
    result.issues.push({ code: "invalid", scheme: "" });
    return result;
  }
  if (config.mode === "scheme" && !config.scheme) {
    result.issues.push({ code: "invalid", scheme: "" });
    return result;
  }
  const destinations = new Set<string>();
  const disabled = new Set(options.disabledHeaders?.map((name) => name.toLowerCase()));
  for (const name of selectedAuthNames(config, requirements)) {
    const scheme = schemes.find((entry) => entry.name === name)?.scheme;
    if (!scheme || !authSchemeSupported(scheme)) {
      result.issues.push({ code: "unsupported", scheme: name });
      continue;
    }
    const isApiKey = scheme.type === "apiKey";
    const destination = isApiKey ? String(scheme.in) : "header";
    const target = isApiKey ? String(scheme.name) : "Authorization";
    if ((destination === "header" && disabled.has(target.toLowerCase())) || (destination === "cookie" && disabled.has("cookie"))) continue;
    const key = `${destination}:${destination === "header" ? target.toLowerCase() : target}`;
    if (destinations.has(key)) {
      result.issues.push({ code: "conflict", scheme: name });
      continue;
    }
    destinations.add(key);
    const input = Object.hasOwn(config.inputs, name) ? config.inputs[name]! : emptyAuthInput();
    let value: string | undefined;
    if (scheme.type === "http" && String(scheme.scheme).toLowerCase() === "basic") {
      const username = inputValue(input.username, variables, authVariableNames(name, "username", scheme));
      const password = inputValue(input.password, variables, authVariableNames(name, "password", scheme));
      if (preview) value = "Basic {{BASE64_CREDENTIALS}}";
      else if (
        (username !== undefined || password !== undefined) &&
        (!input.username || username !== undefined) &&
        (!input.password || password !== undefined) &&
        !(username ?? "").includes(":") &&
        !/[\r\n]/.test((username ?? "") + (password ?? "")) &&
        !((username ?? "") + (password ?? "")).includes("{{")
      )
        value = `Basic ${btoa(Array.from(new TextEncoder().encode(`${username ?? ""}:${password ?? ""}`), (byte) => String.fromCharCode(byte)).join(""))}`;
    } else {
      const token = options.tokens && Object.hasOwn(options.tokens, name) ? options.tokens[name] : undefined;
      const credential = inputValue(input.value, variables, authVariableNames(name, "value", scheme));
      value = preview ? (isAuthTemplate(input.value) ? input.value : isApiKey ? "{{API_KEY}}" : "{{TOKEN}}") : credential;
      if (!isApiKey && value) value = `Bearer ${value}`;
      if (!preview && token) value = token;
    }
    if (destination === "header") {
      result.targets.headers.push(target);
      // Explicit request headers take precedence over inherited/generated authentication.
      if (usable(result.headers.get(target) ?? undefined)) continue;
      if (!preview && !usable(value)) {
        result.issues.push({ code: "missing", scheme: name });
        continue;
      }
      try {
        result.headers.set(target, value!);
      } catch {
        result.issues.push({ code: "invalid", scheme: name });
      }
    } else if (destination === "query") {
      result.targets.query.push(target);
      if (!preview && !usable(value)) {
        result.issues.push({ code: "missing", scheme: name });
        continue;
      }
      result.url = replaceQuery(result.url, target, value!);
    } else {
      result.targets.cookies.push(target);
      if (!preview && !usable(value)) {
        result.issues.push({ code: "missing", scheme: name });
        continue;
      }
      const pairs = (result.headers.get("cookie") ?? "")
        .split(";")
        .map((pair) => pair.trim())
        .filter((pair) => pair && pair.split("=", 1)[0] !== target);
      pairs.push(`${target}=${encodeURIComponent(value!)}`);
      result.headers.set("cookie", pairs.join("; "));
    }
  }
  return result;
}

/** Protect custom credential names as well as conventional Authorization/token names. */
export function redactAuthRequest(request: WorkbenchRequest, targets: OpenApiAuthTargets): WorkbenchRequest {
  let url = request.url;
  for (const name of targets.query) {
    try {
      const value = new URL(url, "https://placeholder.invalid").searchParams.get(name);
      if (value !== null && !isAuthTemplate(value)) url = replaceQuery(url, name, "{{redacted}}");
    } catch {
      /* Leave incomplete URL edits available. */
    }
  }
  const protectedHeaders = new Set(targets.headers.map((name) => name.toLowerCase()));
  if (targets.cookies.length) protectedHeaders.add("cookie");
  return {
    ...request,
    url,
    headers: request.headers.map((row) => (protectedHeaders.has(row.name.toLowerCase()) ? { ...row, value: isAuthTemplate(row.value) ? row.value : "{{redacted}}" } : row)),
    cookies: request.cookies.map((row) => (targets.cookies.includes(row.name) ? { ...row, value: isAuthTemplate(row.value) ? row.value : "{{redacted}}" } : row)),
    ...(request.auth ? { auth: sanitizeAuthConfig(request.auth) } : {}),
  };
}
