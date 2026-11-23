import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useEnvironmentContextStore } from "./environmentContextStore";
import { installMemoryStorage } from "@/test/browserStorage";

beforeEach(() => {
  installMemoryStorage();
  setActivePinia(createPinia());
});

describe("environment context registry", () => {
  it("keeps options from multiple providers and removes only the provider that unmounts", () => {
    const context = useEnvironmentContextStore();
    context.registerOptions("global", [{ id: "local", name: "Local", scope: "global" }]);
    context.registerOptions("openapi:one", [{ id: "staging", name: "Staging", scope: "openapi" }]);

    expect(context.options).toEqual([
      { id: "local", name: "Local", scope: "global" },
      { id: "staging", name: "Staging", scope: "openapi" },
    ]);

    context.unregisterOptions("openapi:one");
    expect(context.options).toEqual([{ id: "local", name: "Local", scope: "global" }]);
  });

  it("deduplicates IDs while preserving the first registered provider", () => {
    const context = useEnvironmentContextStore();
    context.registerOptions("first", [{ id: "shared", name: "First", scope: "global" }]);
    context.registerOptions("second", [{ id: "shared", name: "Second", scope: "openapi" }]);

    expect(context.options).toEqual([{ id: "shared", name: "First", scope: "global" }]);
  });
  it("keeps toolbar options scoped to the active service when an inactive provider updates", () => {
    const context = useEnvironmentContextStore();
    context.activateSource("openapi:one");
    context.registerOptions("openapi:one", [{ id: "qa", name: "QA", scope: "openapi" }], "qa");
    context.registerOptions("openapi:two", [{ id: "prod", name: "Production", scope: "openapi" }], "prod");
    expect(context.activeId).toBe("qa");
    expect(context.activeOptions.map((item) => item.id)).toEqual(["qa"]);
    context.activateSource("openapi:two");
    expect(context.activeId).toBe("prod");
    context.select(null);
    context.activateSource("openapi:one");
    expect(context.activeId).toBe("qa");
    context.activateSource("openapi:two");
    expect(context.activeId).toBe("prod");
  });
  it("repairs a legacy empty OpenAPI selection using the valid default, then the first available environment", () => {
    localStorage.setItem("specx-scoped-environment-selections", JSON.stringify({ "openapi:one": null }));
    const context = useEnvironmentContextStore();
    const qa = { id: "qa", name: "QA", scope: "openapi" } as const;
    const production = { id: "prod", name: "Production", scope: "openapi" } as const;
    context.registerOptions("openapi:one", [qa, production], "prod");
    expect(context.selectionFor("openapi:one")).toBe("prod");
    context.registerOptions("openapi:one", [qa], "prod");
    expect(context.selectionFor("openapi:one")).toBe("qa");
    context.registerOptions("openapi:one", []);
    expect(context.selectionFor("openapi:one")).toBeNull();
  });
  it("saves global drafts without activating them and never persists sensitive values", () => {
    const context = useEnvironmentContextStore();
    const environment = context.addGlobal({ name: "Local", activate: false, variables: [{ key: "credential", value: "private", sensitive: true, enabled: true }] });
    expect(context.activeGlobalId).toBeNull();
    expect(localStorage.getItem("specx-global-environments")).not.toContain("private");
    context.selectGlobal(environment.id);
    expect(context.activeGlobal?.variables[0]?.value).toBe("private");
  });
});
