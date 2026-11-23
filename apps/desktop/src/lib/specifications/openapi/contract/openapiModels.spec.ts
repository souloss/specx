import { describe, expect, it } from "vitest";
import { deleteOpenApiModel, getOpenApiModels, getOpenApiModelUsages, openApiModelDeletionBlockers, openApiModelForReference, updateOpenApiModel, validateOpenApiModelName } from "./openapiModels";
import { openApiPointer, openApiPointerValue } from "./openapiReferences";
import type { OpenApiObject } from "./openapi";

function fixture(): OpenApiObject {
  return {
    openapi: "3.1.0",
    info: { title: "Models", version: "1" },
    paths: {
      "/pets": { parameters: [{ name: "filter", in: "query", schema: { $ref: "#/components/schemas/Filter" } }], get: { responses: { "200": { $ref: "#/components/responses/Pets" } } }, post: { requestBody: { content: { "application/json": { schema: { $ref: "#/components/schemas/NewPet" } } } } } },
    },
    components: {
      schemas: {
        NewPet: { type: "object", properties: { name: { type: "string" }, default: { $ref: "#/components/schemas/Filter" } }, example: { $ref: "#/components/schemas/NewPet" } },
        Pet: { allOf: [{ $ref: "#/components/schemas/NewPet" }, { type: "object", properties: { id: { type: "integer" } } }] },
        Filter: { type: "string" },
        Node: { type: "object", properties: { left: { $ref: "#/components/schemas/Node" }, right: { $ref: "#/components/schemas/Node" } } },
        Unused: { type: "object", default: { $ref: "#/components/schemas/NewPet" }, "x-contract": { $ref: "#/components/schemas/NewPet" } },
      },
      responses: { Pets: { description: "OK", content: { "application/json": { schema: { type: "array", items: { $ref: "#/components/schemas/Pet" } }, examples: { demo: { value: { $ref: "#/components/schemas/NewPet" } } } } } } },
    },
  };
}

describe("OpenAPI shared models", () => {
  it("lists models including booleans and escaped legacy names, and resolves field references to their owner", () => {
    const document = { components: { schemas: { Any: true, Never: false, Bad: 42 } }, definitions: { "A/~ 金": { type: "object" } } };
    expect(getOpenApiModels(document).map((model) => model.name)).toEqual(["A/~ 金", "Any", "Never"]);
    const ref = openApiPointer(["definitions", "A/~ 金", "properties", "name"]);
    expect(openApiModelForReference(document, ref)).toMatchObject({ name: "A/~ 金", reference: "#/definitions/A~1~0%20%E9%87%91" });
    expect(openApiModelForReference(document, "#/components/schemas/Never")).toMatchObject({ schema: false });
    expect(openApiModelForReference(document, "#/components/schemas/%invalid")).toBeUndefined();
    expect(openApiModelForReference(document, "https://example.test/schema")).toBeUndefined();
  });

  it("finds direct and indirect API/model usages through response refs and inherited parameters without cycles or literal examples", () => {
    const document = fixture();
    expect(getOpenApiModelUsages(document, "#/components/schemas/NewPet")).toEqual([
      { kind: "model", key: "#/components/schemas/Pet", title: "Pet", direct: true },
      { kind: "operation", key: "GET:/pets", title: "GET /pets", direct: false },
      { kind: "operation", key: "POST:/pets", title: "POST /pets", direct: true },
    ]);
    expect(getOpenApiModelUsages(document, "#/components/schemas/Filter").map((usage) => usage.key)).toEqual(["#/components/schemas/NewPet", "#/components/schemas/Pet", "GET:/pets", "POST:/pets"]);
    expect(getOpenApiModelUsages(document, "#/components/schemas/Node")).toEqual([]);
  });

  it("renames references and subfield pointers while preserving examples, defaults, extensions and unrelated content", () => {
    const document = fixture();
    const components = document.components as OpenApiObject;
    const schemas = components.schemas as OpenApiObject;
    schemas.Alias = { $ref: "#%2Fcomponents%2Fschemas%2FNewPet" };
    schemas.Branch = { discriminator: { propertyName: "kind", mapping: { pet: "NewPet", full: "#/components/schemas/NewPet" } }, properties: { name: { $ref: "#/components/schemas/NewPet/properties/name" } } };
    const before = JSON.stringify(document);
    const next = updateOpenApiModel(document, "#/components/schemas/NewPet", "PetInput", schemas.NewPet);
    expect(JSON.stringify(document)).toBe(before);
    expect(openApiPointerValue(next, "#/components/schemas/Alias/$ref")).toBe("#/components/schemas/PetInput");
    expect(openApiPointerValue(next, "#/components/schemas/Pet/allOf/0/$ref")).toBe("#/components/schemas/PetInput");
    expect(openApiPointerValue(next, "#/components/schemas/Branch/properties/name/$ref")).toBe("#/components/schemas/PetInput/properties/name");
    expect(openApiPointerValue(next, "#/components/schemas/Branch/discriminator/mapping")).toEqual({ pet: "PetInput", full: "#/components/schemas/PetInput" });
    expect(openApiPointerValue(next, "#/components/schemas/PetInput/example/$ref")).toBe("#/components/schemas/NewPet");
    expect(openApiPointerValue(next, "#/components/schemas/Unused")).toEqual(schemas.Unused);
    expect(openApiPointerValue(next, "#/components/responses/Pets/content/application~1json/examples/demo/value/$ref")).toBe("#/components/schemas/NewPet");
    expect(openApiPointerValue(next, "#/components/schemas/NewPet")).toBeUndefined();
  });

  it("renames recursive references without mutating the caller's schema", () => {
    const document = fixture();
    const schema = ((document.components as OpenApiObject).schemas as OpenApiObject).Node;
    const before = JSON.stringify(document);
    const next = updateOpenApiModel(document, "#/components/schemas/Node", "TreeNode", schema);
    expect(JSON.stringify(document)).toBe(before);
    expect(openApiPointerValue(next, "#/components/schemas/TreeNode/properties/left/$ref")).toBe("#/components/schemas/TreeNode");
    expect(openApiPointerValue(next, "#/components/schemas/TreeNode/properties/right/$ref")).toBe("#/components/schemas/TreeNode");
  });

  it("rejects duplicates, invalid names, missing models and boolean schemas before OpenAPI 3.1", () => {
    const document = fixture();
    expect(validateOpenApiModelName(document, "Pet")).toBe("duplicate");
    expect(validateOpenApiModelName(document, "Pet", "#/components/schemas/Pet")).toBeNull();
    expect(validateOpenApiModelName(document, "Pet Input")).toBe("invalidName");
    expect(() => updateOpenApiModel(document, "#/components/schemas/Missing", "Missing", {})).toThrow("missing");
    expect(() => updateOpenApiModel({ openapi: "3.0.3" }, undefined, "Any", true)).toThrow("invalidSchema");
    expect(openApiPointerValue(updateOpenApiModel(document, undefined, "Never", false), "#/components/schemas/Never")).toBe(false);
  });

  it("blocks removal of references anywhere in the contract, but permits deleting a self recursive unused model", () => {
    const document = fixture();
    (document.components as OpenApiObject).requestBodies = { UnusedBody: { content: { "application/json": { schema: { $ref: "#/components/schemas/Unused" } } } } };
    expect(openApiModelDeletionBlockers(document, "#/components/schemas/Unused")).toHaveLength(1);
    expect(() => deleteOpenApiModel(document, "#/components/schemas/Unused")).toThrow("inUse");
    expect(openApiModelDeletionBlockers(document, "#/components/schemas/Node")).toEqual([]);
    expect(getOpenApiModels(deleteOpenApiModel(document, "#/components/schemas/Node")).some((model) => model.name === "Node")).toBe(false);
    expect(getOpenApiModels(document).some((model) => model.name === "Node")).toBe(true);
  });

  it("treats property and model names as names, including x- prefixes and __proto__", () => {
    const document = { openapi: "3.1.0", components: { schemas: { A: {}, "x-Model": { properties: { "x-data": { $ref: "#/components/schemas/A" } } } } } };
    const next = updateOpenApiModel(document, "#/components/schemas/A", "__proto__", { type: "string" });
    expect(openApiPointerValue(next, "#/components/schemas/x-Model/properties/x-data/$ref")).toBe("#/components/schemas/__proto__");
    expect(openApiPointerValue(next, "#/components/schemas/__proto__")).toEqual({ type: "string" });
    expect(Object.getPrototypeOf((next.components as OpenApiObject).schemas)).toBe(Object.prototype);
  });
});
