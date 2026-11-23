import { describe, expect, it } from "vitest";
import { extractPathParameterNames, resolvePathTemplate, restorePathTemplate } from "./openapiPathTemplate";
describe("path template execution", () => {
  it("extracts unique route parameters, excluding environment variables and query values", () => {
    expect(extractPathParameterNames("{{BASE}}/pets/{petId}/{petId}/{tag}?q={query}")).toEqual(["petId", "tag"]);
    expect(extractPathParameterNames("https://example.test/{a-b}")).toEqual(["a-b"]);
  });
  it("substitutes encoded values without altering the displayed template or environment variables", () => {
    expect(resolvePathTemplate("{{BASE}}/pets/{id}?q={q}", { id: "a/b?c", q: "no" })).toBe("{{BASE}}/pets/a%2Fb%3Fc?q={q}");
    expect(resolvePathTemplate("/pets/{missing}", {})).toBe("/pets/{missing}");
  });
  it("restores a matching operation template while retaining a custom target and query", () => {
    expect(restorePathTemplate("https://other.test/api/v2/accounts/a%2Fb?view=full", "/accounts/{accountId}")).toBe("https://other.test/api/v2/accounts/{accountId}?view=full");
    expect(restorePathTemplate("https://other.test/pets/42", "/accounts/{accountId}")).toBe("https://other.test/pets/42");
    expect(restorePathTemplate("/accounts/{accountId}?view=full", "/accounts/{accountId}")).toBe("/accounts/{accountId}?view=full");
  });
});
