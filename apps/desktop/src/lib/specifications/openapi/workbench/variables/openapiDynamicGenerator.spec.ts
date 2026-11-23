import { describe, expect, it } from "vitest";
import { buildDynamicGeneratorExpression } from "./openapiDynamicGenerator";
import { createOpenApiVariableResolver, dynamicExpressionRangeAt } from "./openapiDynamicVariables";

describe("OpenAPI data generator", () => {
  it("passes typed generator arguments and transforms to request resolution", () => {
    const expression = buildDynamicGeneratorExpression("helpers.arrayElement", { array: '[" a|b "]' }, [
      { name: "trim", value: "" },
      { name: "upperCase", value: "" },
    ]);
    expect(createOpenApiVariableResolver().expand(expression)).toBe("A|B");
    expect(createOpenApiVariableResolver().expandJson(`{"value":"${expression}"}`)).toBe('{"value":"A|B"}');
    expect(() => buildDynamicGeneratorExpression("number.int", { min: "abc" }, [])).toThrow();
    expect(() => buildDynamicGeneratorExpression("date.now", {}, [{ name: "addDays", value: "" }])).toThrow();
  });

  it("selects an existing expression without consuming surrounding input", () => {
    const expression = '{{$helpers.arrayElement(["}}"])|upperCase()}}';
    const input = `https://example.test/${expression}/next`;
    const start = input.indexOf(expression);
    for (const cursor of [start + 10, start + expression.length]) {
      const range = dynamicExpressionRangeAt(input, cursor)!;
      expect(input.slice(range.from, range.to)).toBe(expression);
    }
    expect(dynamicExpressionRangeAt(input, input.length)).toBeNull();
  });
});
