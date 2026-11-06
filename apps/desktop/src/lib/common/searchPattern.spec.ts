import { describe, expect, it } from "vitest";
import { compileSearchRegex } from "./searchPattern";

describe("compileSearchRegex", () => {
  it("compiles plain values case-insensitively", () => {
    const result = compileSearchRegex("billing");
    expect(result.invalid).toBe(false);
    expect(result.regex?.test("Billing API")).toBe(true);
  });

  it("accepts slash-delimited expressions and flags", () => {
    const result = compileSearchRegex("/^billing-/i");
    expect(result.invalid).toBe(false);
    expect(result.regex?.test("billing-api")).toBe(true);
  });

  it("reports invalid expressions without throwing", () => {
    expect(compileSearchRegex("[")).toEqual({ regex: null, invalid: true });
  });
});
