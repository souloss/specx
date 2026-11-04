import { createI18n } from "vue-i18n";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import en from "./locales/en";
import zhCN, { type MessageSchema } from "./locales/zh-CN";

export type AppLocale = "zh-CN" | "en";
type MessagePaths<T> = { [K in keyof T & string]: T[K] extends string ? K : T[K] extends Record<string, unknown> ? `${K}.${MessagePaths<T[K]>}` : never }[keyof T & string];
export type MessageKey = MessagePaths<MessageSchema>;
export const LOCALE_STORAGE_KEY = "specx-locale";
const supportedLocales: AppLocale[] = ["zh-CN", "en"];
const savedLocale = safeLocalStorageGet(LOCALE_STORAGE_KEY);
const initialLocale: AppLocale = supportedLocales.includes(savedLocale as AppLocale) ? (savedLocale as AppLocale) : "zh-CN";
if (typeof document !== "undefined") document.documentElement.lang = initialLocale;

export const i18n = createI18n({
  legacy: false,
  locale: initialLocale,
  fallbackLocale: "zh-CN",
  messages: { "zh-CN": zhCN, en },
});

export function setLocale(locale: AppLocale) {
  i18n.global.locale.value = locale;
  safeLocalStorageSet(LOCALE_STORAGE_KEY, locale);
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}

export function translate(key: MessageKey, params?: Record<string, string | number>) {
  return i18n.global.t(key, params ?? {});
}
