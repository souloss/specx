import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { installMemoryStorage } from "@/test/browserStorage";
import { resolveEnvironmentVariableValues, useEnvironmentLocalValues } from "./environmentLocalValues";
beforeEach(() => {
  installMemoryStorage();
  setActivePinia(createPinia());
});
describe("environment local values", () => {
  it("isolates newly created script variables by environment and keeps them out of persistent storage", () => {
    const store = useEnvironmentLocalValues();
    store.replaceScriptValues("workspace:service:qa", { cursor: "42" });
    expect(store.scriptValues("workspace:service:qa")).toEqual({ cursor: "42" });
    expect(store.scriptValues("workspace:service:prod")).toEqual({});
    expect(store.values("workspace:service:qa")).toEqual({});
    setActivePinia(createPinia());
    expect(useEnvironmentLocalValues().scriptValues("workspace:service:qa")).toEqual({});
  });
  it("keeps empty overrides distinct from following the remote value and isolates scope", () => {
    const store = useEnvironmentLocalValues();
    store.replace("workspace:service:qa", [{ key: "region", localValue: "", sensitive: false, enabled: true }]);
    expect(store.values("workspace:service:qa")).toEqual({ region: "" });
    expect(store.values("other:service:qa")).toEqual({});
    store.replace("workspace:service:qa", [{ key: "region", sensitive: false, enabled: true }]);
    expect(store.values("workspace:service:qa")).toEqual({});
  });
  it("persists ordinary values while keeping secrets only for the current session", () => {
    const store = useEnvironmentLocalValues();
    store.replace("qa", [
      { key: "region", localValue: "east", sensitive: false, enabled: true },
      { key: "token", localValue: "secret", sensitive: true, enabled: true },
    ]);
    expect(store.values("qa")).toEqual({ region: "east", token: "secret" });
    setActivePinia(createPinia());
    expect(useEnvironmentLocalValues().values("qa")).toEqual({ region: "east" });
  });
  it("applies local values only to enabled, declared variables and lets later environment scopes win", () => {
    const global = resolveEnvironmentVariableValues(
      [
        { key: "host", value: "https://global", sensitive: false, enabled: true },
        { key: "disabled", value: "remote", sensitive: false, enabled: false },
        { key: "secret", value: "remote-secret", sensitive: true, enabled: true },
      ],
      { host: "https://local", disabled: "ignored", stale: "ignored", secret: "local-secret" },
      false,
    );
    const service = resolveEnvironmentVariableValues([{ key: "host", value: null, sensitive: false, enabled: true }], { host: "https://service-local" });
    expect({ ...global, ...service }).toEqual({ host: "https://service-local" });
  });
});
