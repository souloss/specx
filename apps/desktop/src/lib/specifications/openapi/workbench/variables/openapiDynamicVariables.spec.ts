import examples from "./__fixtures__/apifoxDynamicExpressions.json";
import { describe, expect, it } from "vitest";
import { createOpenApiVariableResolver, evaluateDynamicExpression, OPENAPI_DYNAMIC_VARIABLES } from "./openapiDynamicVariables";

describe("Apifox dynamic expressions", () => {
  it.each(examples)("evaluates the documented expression %s", (expression) => {
    expect(evaluateDynamicExpression(expression)).not.toBeUndefined();
  });
  it("supports UUID, timestamps, Faker namespaces and named literal options", () => {
    expect(evaluateDynamicExpression("$string.uuid")).toMatch(/^[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}$/);
    expect(evaluateDynamicExpression("$number.int(min=12,max=12)")).toBe(12);
    expect(evaluateDynamicExpression('$string.alpha(length=8,casing="upper")')).toMatch(/^[A-Z]{8}$/);
    expect(evaluateDynamicExpression("$timestamp", { now: new Date("2026-01-01T00:00:00Z") })).toBe(1767225600);
    expect(evaluateDynamicExpression('$person.fullName(locale="en")')).toMatch(/[A-Za-z]/);
    expect(OPENAPI_DYNAMIC_VARIABLES.length).toBeGreaterThan(250);
  });
  it("handles quoted delimiters, array helpers, projections, date pipes and localized generators", () => {
    expect(evaluateDynamicExpression("$helpers.arrayElement(['a,b|c'])")).toBe("a,b|c");
    expect(evaluateDynamicExpression("$airline.airportIataCode")).toMatch(/^[A-Z]{3}$/);
    expect(evaluateDynamicExpression("$date.now|addDays(7)|format('yyyy-MM-dd')", { now: new Date(2026, 0, 1) })).toBe("2026-01-08");
    expect(evaluateDynamicExpression("$phone.mobile")).toMatch(/^1[35789]\d{9}$/);
    expect(evaluateDynamicExpression("$person.idCard")).toMatch(/^\d{17}[\dX]$/);
  });
  it("resolves nested variables, retains unknown static variables and detects cycles", () => {
    const resolver = createOpenApiVariableResolver();
    expect(resolver.expand("{{a}}/{{missing}}", { a: "{{b}}", b: "{{$number.int(min=7,max=7)}}" })).toBe("7/{{missing}}");
    expect(() => resolver.expand("{{a}}", { a: "{{b}}", b: "{{a}}" })).toThrow("cycle");
    expect(() => resolver.expand("{{$unknown.method}}")).toThrow("unknown");
  });
  it("preserves JSON types, escapes quotes and generates fresh values on each send", () => {
    const resolver = createOpenApiVariableResolver();
    const body = resolver.expandJson('{"id":{{$number.int(min=7,max=7)}},"enabled":{{$datatype.boolean(probability=1)}},"name":"{{name}}"}', { name: 'a"b\nc' });
    expect(JSON.parse(resolver.expandJson(`{"name":"{{$helpers.arrayElement(['a"b'])}}"}`))).toEqual({ name: 'a"b' });
    expect(JSON.parse(body)).toEqual({ id: 7, enabled: true, name: 'a"b\nc' });
    expect(resolver.expand("{{$string.uuid}}")).not.toBe(resolver.expand("{{$string.uuid}}"));
  });
  it("increments a named environment variable and publishes its update to its owning scope", () => {
    const updates: Record<string, string> = {};
    const variables = { counter: "4" };
    const resolver = createOpenApiVariableResolver({
      onVariableUpdate: (name, value) => {
        updates[name] = value;
      },
    });
    expect(resolver.expand('{{$helper.autoIncrementEnvVar("counter")}}', variables)).toBe("5");
    expect(resolver.expand('{{$helper.autoIncrementEnvVar("counter")}}', variables)).toBe("6");
    expect(updates).toEqual({ counter: "6" });
  });
  it("rejects executable arguments, prototype access and excessive generation", () => {
    for (const expr of ["$number.int(min=process.exit())", '$string.constructor.constructor("alert(1)")', '$helpers.fromRegExp("[a-z]{999999}")', "$string.alpha(length=1000000)", "$string.nanoid(1000000000)", "$string.nanoid(length={min:1,max:1000000000})"])
      expect(() => evaluateDynamicExpression(expr)).toThrow();
  });
});
