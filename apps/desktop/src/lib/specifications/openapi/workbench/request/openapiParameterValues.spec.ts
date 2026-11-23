import { describe, it, expect } from "vitest";
import { expandOpenApiParameterValue } from "./openapiParameterValues";
import { createOpenApiVariableResolver } from "../variables/openapiDynamicVariables";
describe("typed structured parameters", () => {
  it("resolves references, nested arrays and dynamic typed values without damaging strings", () => {
    const document = {
      components: {
        schemas: { Filter: { type: "object", properties: { active: { type: "boolean" }, count: { type: "integer" }, names: { type: "array", items: { type: "string" } }, child: { $ref: "#/components/schemas/Child" } } }, Child: { type: "object", properties: { enabled: { type: "boolean" } } } },
      },
    };
    const result = expandOpenApiParameterValue('{"active":"{{$datatype.boolean(probability=1)}}","count":"{{$number.int(min=7,max=7)}}","names":["{{name}}"],"child":{"enabled":"false"}}', { $ref: "#/components/schemas/Filter" }, document, createOpenApiVariableResolver(), { name: 'a"b' });
    expect(expandOpenApiParameterValue(JSON.stringify({ names: ['{{$person.fullName(locale="en")}}'] }), { $ref: "#/components/schemas/Filter" }, document, createOpenApiVariableResolver(), {})).not.toContain("{{");
    expect(JSON.parse(result)).toEqual({ active: true, count: 7, names: ['a"b'], child: { enabled: false } });
  });
});
