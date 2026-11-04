import { i18n, translate, type MessageKey } from "@/i18n";
import zhCN from "@/i18n/locales/zh-CN";

export type RequestOptions = {
  method?: string;
  headers?: Record<string, string>;
  body?: unknown;
};

export type BackendTransport = {
  request<T>(path: string, options?: RequestOptions): Promise<T>;
};

type MessageParams = Record<string, string | number>;
type ErrorIdentity = {
  code: string;
  messageKey: MessageKey;
  messageParams: MessageParams;
};

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function errorIdentity(body: unknown): ErrorIdentity | undefined {
  if (!record(body)) return;
  if (body.version !== 1) return;
  if (typeof body.code !== "string" || !/^[a-z][a-z0-9_]{0,79}$/.test(body.code)) return;
  if (typeof body.messageKey !== "string" || !body.messageKey.startsWith("apiError.")) return;
  if (!record(body.messageParams)) return;

  const name = body.messageKey.slice("apiError.".length);
  if (!Object.hasOwn(zhCN.apiError, name)) return;
  const template = zhCN.apiError[name as keyof typeof zhCN.apiError];
  const placeholders = [...template.matchAll(/\{([a-zA-Z][a-zA-Z0-9_]*)\}/g)].map((match) => match[1]);
  const expected = [...new Set(placeholders)].sort();
  const params = body.messageParams;
  const names = Object.keys(params).sort();
  if (names.length !== expected.length || names.some((name, index) => name !== expected[index])) return;
  const validValues = Object.values(params).every((value) => (typeof value === "string" && value.length <= 512) || (typeof value === "number" && Number.isFinite(value)));
  if (!validValues) return;
  return {
    code: body.code,
    messageKey: body.messageKey as MessageKey,
    messageParams: params as MessageParams,
  };
}

export class ApiError extends Error {
  readonly code?: string;
  readonly messageKey?: MessageKey;
  readonly messageParams?: MessageParams;

  constructor(
    message: string,
    readonly status: number,
    identity?: ErrorIdentity,
  ) {
    super(message);
    this.name = "ApiError";
    this.code = identity?.code;
    this.messageKey = identity?.messageKey;
    this.messageParams = identity?.messageParams;
  }

  static fromResponse(body: unknown, status: number): ApiError {
    const identity = errorIdentity(body);
    const locale = i18n.global.locale.value;
    const key = identity?.messageKey;
    const exists = key && (i18n.global.te(key, locale) || i18n.global.te(key, "zh-CN"));
    const message = exists ? translate(key, identity.messageParams) : translate("apiError.requestFailed", { status });
    return new ApiError(message, status, identity);
  }
}

export function errorMessage(body: unknown, status: number): string {
  return ApiError.fromResponse(body, status).message;
}
