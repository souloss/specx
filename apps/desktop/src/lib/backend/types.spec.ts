import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import catalog from "../../../../../crates/specx-api/tests/fixtures/error-catalog.json";
import { i18n, setLocale, translate } from "@/i18n";
import { ApiError, errorMessage } from "./types";
import { httpTransport } from "./http";
import { desktopTransport } from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const envelope = {
  version: 1,
  code: "precondition_failed",
  messageKey: "apiError.preconditionFailed",
  messageParams: {},
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
  setLocale("zh-CN");
});

describe("API error contract", () => {
  it("renders every backend catalog entry in both locales and preserves machine identity", () => {
    for (const locale of ["zh-CN", "en"] as const) {
      setLocale(locale);
      for (const { status, body } of catalog) {
        expect(i18n.global.te(body.messageKey, locale), body.messageKey).toBe(true);
        const error = ApiError.fromResponse(body, status);
        expect(error.status).toBe(status);
        expect(error.code).toBe(body.code);
        expect(error.messageKey).toBe(body.messageKey);
        expect(error.messageParams).toEqual({});
        expect(error.message).not.toBe(body.messageKey);
        expect(error.message).not.toContain("secret");
        expect(error.message).not.toBe(translate("apiError.requestFailed", { status }));
      }
    }
  });

  it("uses named interpolation after validating parameter names and types", () => {
    setLocale("en");
    const error = ApiError.fromResponse(
      {
        messageKey: "apiError.requestFailed",
        code: "request_failed",
        version: 1,
        messageParams: { status: 409 },
      },
      409,
    );
    expect(error.message).toBe("Request failed (409)");
    expect(error.messageKey).toBe("apiError.requestFailed");
  });

  it("uses the fallback locale for a key missing from the current locale", () => {
    const messages = i18n.global.getLocaleMessage("en");
    const incomplete = { ...messages, apiError: { ...messages.apiError } };
    Reflect.deleteProperty(incomplete.apiError, "preconditionFailed");
    i18n.global.setLocaleMessage("en", incomplete);
    setLocale("en");
    try {
      expect(errorMessage(envelope, 412)).toBe("其他人已修改此资源，请刷新并确认更改后重试。");
    } finally {
      i18n.global.setLocaleMessage("en", messages);
    }
  });

  it("rejects unversioned legacy responses", () => {
    setLocale("en");
    expect(errorMessage({ code: "invalid_credentials", message: "private SQL" }, 401)).toBe("Request failed (401)");
    expect(errorMessage({ code: "unknown_code", message: "private SQL" }, 500)).toBe("Request failed (500)");
  });

  it.each([
    null,
    [],
    "private SQL",
    {},
    { ...envelope, version: 2 },
    { ...envelope, version: "1" },
    { ...envelope, version: undefined },
    { ...envelope, code: 1 },
    { ...envelope, messageKey: undefined },
    { ...envelope, messageKey: "apiError.unknown" },
    { ...envelope, messageKey: "common.delete" },
    { ...envelope, messageKey: "apiError.__proto__" },
    { ...envelope, messageParams: undefined },
    { ...envelope, messageParams: [] },
    { ...envelope, messageParams: { unexpected: "private SQL" } },
    { ...envelope, messageKey: "apiError.requestFailed", messageParams: {} },
    { ...envelope, messageKey: "apiError.requestFailed", messageParams: { status: {} } },
    { ...envelope, messageKey: "apiError.requestFailed", messageParams: { status: Infinity } },
    { ...envelope, messageKey: "apiError.requestFailed", messageParams: { status: NaN } },
    { ...envelope, messageKey: "apiError.requestFailed", messageParams: { status: "s".repeat(513) } },
  ])("uses a safe localized fallback for an invalid envelope: %j", (body) => {
    setLocale("en");
    const error = ApiError.fromResponse(body, 500);
    expect(error.message).toBe("Request failed (500)");
    expect(error.code).toBeUndefined();
  });
});

describe.each([
  ["HTTP", httpTransport],
  ["Tauri", desktopTransport],
] as const)("%s error transport", (name, transport) => {
  it("preserves the same error identity and localized summary", async () => {
    setLocale("en");
    if (name === "HTTP") {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify(envelope), { status: 412 })));
    } else {
      vi.mocked(invoke).mockResolvedValue({ status: 412, body: envelope, headers: {} });
    }
    await expect(transport.request("/api/specx/services/one")).rejects.toMatchObject({
      name: "ApiError",
      status: 412,
      code: "precondition_failed",
      messageKey: "apiError.preconditionFailed",
      message: "Someone changed this item. Refresh and review the changes before trying again.",
    });
  });
});
