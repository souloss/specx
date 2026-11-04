import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { errorMessage } from "@/lib/backend/types";
import { i18n, LOCALE_STORAGE_KEY, setLocale, translate } from "@/i18n";
import en from "@/i18n/locales/en";
import zhCN from "@/i18n/locales/zh-CN";
import { installMemoryStorage } from "@/test/browserStorage";

function flattenMessages(messages: Record<string, unknown>, prefix = ""): Record<string, string> {
  const flattened: Record<string, string> = {};
  for (const [key, value] of Object.entries(messages)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") {
      flattened[path] = value;
    } else {
      Object.assign(flattened, flattenMessages(value as Record<string, unknown>, path));
    }
  }
  return flattened;
}

function placeholders(message: string): string[] {
  return [...new Set([...message.matchAll(/\{([a-zA-Z][a-zA-Z0-9_]*)\}/g)].map((match) => match[1]))].sort();
}

describe("application locale", () => {
  beforeEach(() => {
    installMemoryStorage();
  });
  afterEach(() => {
    setLocale("zh-CN");
  });

  it("switches UI text immediately and persists the selected locale", () => {
    setLocale("en");

    expect(i18n.global.t("toolbar.logout")).toBe("Sign out");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("en");
    expect(document.documentElement.lang).toBe("en");

    setLocale("zh-CN");
    expect(i18n.global.t("toolbar.logout")).toBe("退出登录");
  });

  it("localizes API failures from the versioned message key", () => {
    const invalidCredentials = {
      version: 1,
      code: "invalid_credentials",
      messageKey: "apiError.invalidCredentials",
      messageParams: {},
    };
    setLocale("en");
    expect(errorMessage(invalidCredentials, 401)).toBe("The access password is incorrect.");

    setLocale("zh-CN");
    expect(errorMessage(invalidCredentials, 401)).toBe("访问密码不正确。");
  });

  it("uses English singular and plural service counts", () => {
    setLocale("en");
    expect(translate("sidebar.serviceCount", { count: 0 })).toBe("0 services");
    expect(translate("sidebar.serviceCount", { count: 1 })).toBe("1 service");
    expect(translate("sidebar.serviceCount", { count: 2 })).toBe("2 services");
  });

  it("keeps locale keys and named interpolation placeholders aligned", () => {
    const chinese = flattenMessages(zhCN);
    const english = flattenMessages(en);

    expect(Object.keys(english).sort()).toEqual(Object.keys(chinese).sort());
    for (const key of Object.keys(chinese)) {
      expect(placeholders(english[key]), `${key} placeholders`).toEqual(placeholders(chinese[key]));
    }
  });
});
