import { ref } from "vue";
import { defineStore } from "pinia";
import { safeLocalStorageGet, safeLocalStorageSet } from "@/lib/backend/safeStorage";
import { isRecord } from "@/lib/specifications/openapi/contract/openapi";
export type LocalEnvironmentRow = { key: string; localValue?: string; sensitive: boolean; enabled: boolean };
export type EnvironmentVariableValue = { key: string; value: string | null; sensitive: boolean; enabled: boolean };
const storageKey = "specx-environment-local-values-v1";
function read(): Record<string, Record<string, string>> {
  try {
    const value: unknown = JSON.parse(safeLocalStorageGet(storageKey) ?? "{}");
    if (!isRecord(value)) return {};
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, rows]) => isRecord(rows))
        .map(([scope, rows]) => [scope, Object.fromEntries(Object.entries(rows as Record<string, unknown>).filter((entry): entry is [string, string] => typeof entry[1] === "string"))]),
    );
  } catch {
    return {};
  }
}

export function resolveEnvironmentVariableValues(remote: EnvironmentVariableValue[], local: Record<string, string>, includeSensitive = true): Record<string, string> {
  return Object.fromEntries(
    remote.flatMap((row) => {
      const key = row.key.trim();
      if (!key || !row.enabled || (!includeSensitive && row.sensitive)) return [];
      const value = Object.hasOwn(local, key) ? local[key] : row.value;
      return typeof value === "string" ? [[key, value]] : [];
    }),
  );
}

export const useEnvironmentLocalValues = defineStore("environmentLocalValues", () => {
  const persistent = ref(read());
  const session = ref<Record<string, Record<string, string>>>({});
  const scriptOnly = ref<Record<string, Record<string, string>>>({});
  function values(scope: string) {
    return { ...persistent.value[scope], ...session.value[scope] };
  }
  function replace(scope: string, rows: LocalEnvironmentRow[]) {
    const defined = rows.filter((row) => row.localValue !== undefined);
    persistent.value = { ...persistent.value, [scope]: Object.fromEntries(defined.filter((row) => !row.sensitive).map((row) => [row.key, row.localValue!])) };
    session.value = { ...session.value, [scope]: Object.fromEntries(defined.filter((row) => row.sensitive).map((row) => [row.key, row.localValue!])) };
    safeLocalStorageSet(storageKey, JSON.stringify(persistent.value));
  }
  // Variables created by scripts stay in this client session. They are not
  // silently promoted to shared server-side environment definitions.
  function scriptValues(scope: string) {
    return { ...scriptOnly.value[scope] };
  }
  function replaceScriptValues(scope: string, values: Record<string, string>) {
    scriptOnly.value = { ...scriptOnly.value, [scope]: { ...values } };
  }
  return { values, replace, scriptValues, replaceScriptValues };
});
