import { describe, expect, it } from "vitest";
import { createOpenApiOperation, moveOpenApiOperation, validateOpenApiOperationAddress } from "./openapiOperationEditing";
import { resolveOpenApiRef, type OpenApiObject } from "../contract/openapi";

const document = {
  openapi: "3.1.0",
  info: { title: "Operations", version: "1" },
  paths: {
    "/pets/{id}": {
      servers: [{ url: "https://pets.example.test" }],
      parameters: [
        { name: "id", in: "path", required: true, schema: { type: "integer" } },
        { name: "X-Tenant", in: "header", required: true, schema: { type: "string" } },
      ],
      get: { operationId: "getPet", parameters: [{ name: "id", in: "path", required: true, description: "Pet identifier", schema: { type: "integer", minimum: 1 } }], responses: { "200": { description: "OK", content: { "application/json": { schema: { $ref: "#/components/schemas/Pet" } } } } } },
      delete: { responses: { "204": { description: "Removed" } } },
    },
  },
  components: { schemas: { Pet: { type: "object", properties: { name: { type: "string" } } } } },
};

describe("OpenAPI operation lifecycle transforms", () => {
  it("creates one API, generates required path parameters and preserves existing facts", () => {
    const next = createOpenApiOperation(document, { method: "POST", path: "/orders/{orderId}", summary: " Create order ", tags: ["Orders", "Orders", ""] });
    expect(next).toMatchObject({ paths: { "/orders/{orderId}": { post: { summary: "Create order", tags: ["Orders"], parameters: [{ name: "orderId", in: "path", required: true, schema: { type: "string" } }], responses: { "200": { description: "OK" } } } } } });
    const expected = structuredClone(document) as OpenApiObject;
    (expected.paths as OpenApiObject)["/orders/{orderId}"] = (next.paths as OpenApiObject)["/orders/{orderId}"];
    expect(next).toEqual(expected);
    expect(document.paths).not.toHaveProperty("/orders/{orderId}");
  });

  it.each(["pets", "/pets?limit=1", "/pets#anchor", "/pet name", "/pets/{}", "/pets/{id", "/{id}/{id}", "/pets/{a/b}", "/pets\\name"])("rejects invalid API path %s", (path) => {
    expect(validateOpenApiOperationAddress(document, { method: "get", path })).toBe("path");
  });

  it("rejects duplicate, ambiguous templated and referenced routes", () => {
    expect(validateOpenApiOperationAddress(document, { method: "get", path: "/pets/{id}" })).toBe("duplicate");
    expect(validateOpenApiOperationAddress(document, { method: "post", path: "/pets/{petId}" })).toBe("template");
    expect(validateOpenApiOperationAddress(document, { method: "connect", path: "/tunnel" })).toBe("method");
    expect(validateOpenApiOperationAddress({ paths: { "/pets": { $ref: "#/components/pathItems/Pets" } } }, { method: "get", path: "/pets" })).toBe("reference");
    expect(validateOpenApiOperationAddress(document, { method: "get", path: "/other" }, "GET:/missing")).toBe("missing");
  });

  it("moves only the chosen operation, carries inherited contract facts and remaps its path parameter", () => {
    const next = moveOpenApiOperation(document, "GET:/pets/{id}", { method: "patch", path: "/animals/{petId}" });
    const paths = next.paths as Record<string, OpenApiObject>;
    expect(paths["/pets/{id}"]).toEqual({ servers: document.paths["/pets/{id}"].servers, parameters: document.paths["/pets/{id}"].parameters, delete: document.paths["/pets/{id}"].delete });
    expect(paths["/animals/{petId}"]?.patch).toEqual({ ...document.paths["/pets/{id}"].get, servers: document.paths["/pets/{id}"].servers, parameters: [{ ...document.paths["/pets/{id}"].get.parameters[0], name: "petId" }, document.paths["/pets/{id}"].parameters[1]] });
    expect(next.components).toEqual(document.components);
    expect(document.paths["/pets/{id}"].get.parameters[0]!.name).toBe("id");
  });

  it("generates new placeholders, removes obsolete local path parameters and preserves destination siblings", () => {
    const source = { ...document, paths: { ...document.paths, "/animals/{animalId}/visits/{visitId}": { parameters: [{ name: "animalId", in: "path", required: true, schema: { type: "string", format: "uuid" } }], post: { responses: { "201": { description: "Created" } } } } } };
    const next = moveOpenApiOperation(source, "GET:/pets/{id}", { method: "get", path: "/animals/{animalId}/visits/{visitId}" });
    const destination = (next.paths as Record<string, OpenApiObject>)["/animals/{animalId}/visits/{visitId}"]!;
    expect(destination.post).toEqual(source.paths["/animals/{animalId}/visits/{visitId}"].post);
    expect(destination.parameters).toEqual(source.paths["/animals/{animalId}/visits/{visitId}"].parameters);
    expect((destination.get as OpenApiObject).parameters).toEqual([source.paths["/pets/{id}"].parameters[1], { name: "visitId", in: "path", required: true, schema: { type: "string" } }]);
  });

  it("renames a method without changing its inherited parameters or schemas", () => {
    const next = moveOpenApiOperation(document, "GET:/pets/{id}", { method: "head", path: "/pets/{id}" });
    expect((next.paths as Record<string, OpenApiObject>)["/pets/{id}"]).toEqual({ ...document.paths["/pets/{id}"], get: undefined, head: document.paths["/pets/{id}"].get });
    expect(next.components).toEqual(document.components);
  });

  it("matches unchanged placeholder names before assigning renamed parameters", () => {
    const first = { name: "a", in: "path", required: true, schema: { type: "integer" } };
    const second = { name: "b", in: "path", required: true, schema: { type: "string", format: "uuid" } };
    const source = { paths: { "/{a}/{b}": { get: { parameters: [first, second], responses: { "200": { description: "OK" } } } } } };
    const next = moveOpenApiOperation(source, "GET:/{a}/{b}", { method: "get", path: "/renamed/{b}/{c}" });
    expect(next.paths).toEqual({ "/renamed/{b}/{c}": { get: { parameters: [{ ...first, name: "c" }, second], responses: { "200": { description: "OK" } } } } });
  });

  it("removes an empty source path so a sole operation's placeholder can be renamed", () => {
    const source = { paths: { "/pets/{id}": { get: { parameters: [{ name: "id", in: "path", required: true, schema: { type: "integer" } }], responses: {} } } } };
    const next = moveOpenApiOperation(source, "GET:/pets/{id}", { method: "get", path: "/pets/{petId}" });
    expect(next.paths).not.toHaveProperty("/pets/{id}");
    expect(next.paths).toMatchObject({ "/pets/{petId}": { get: { parameters: [{ name: "petId", in: "path", required: true, schema: { type: "integer" } }] } } });
  });

  it("updates local operation and schema references while preserving literal example and default values", () => {
    const previous = "#/paths/~1pets~1{id}/get";
    const target = "#/paths/~1animals~1{petId}/patch";
    const literal = { $ref: previous, operationRef: previous };
    const source = {
      ...document,
      components: {
        links: { follow: { operationRef: previous } },
        schemas: { example: { $ref: `${previous}/responses/200/content/application~1json/schema` }, value: { properties: { example: { $ref: `${previous}/parameters/0/schema` } }, default: literal, enum: [literal], examples: [literal] } },
        examples: { sample: { value: literal } },
      },
    };
    const next = moveOpenApiOperation(source, "GET:/pets/{id}", { method: "patch", path: "/animals/{petId}" });
    expect(next.components).toEqual({
      links: { follow: { operationRef: target } },
      schemas: { example: { $ref: `${target}/responses/200/content/application~1json/schema` }, value: { properties: { example: { $ref: `${target}/parameters/0/schema` } }, default: literal, enum: [literal], examples: [literal] } },
      examples: { sample: { value: literal } },
    });
  });

  it("keeps references resolvable when route parameters disappear or change array positions", () => {
    const source = {
      openapi: "3.1.0",
      paths: {
        "/old/{id}/{unused}": {
          get: {
            parameters: [
              { name: "id", in: "path", required: true, schema: { type: "integer" } },
              { name: "unused", in: "path", required: true, schema: { type: "number", minimum: 1 } },
              { name: "trace", in: "query", schema: { type: "string", format: "uuid" } },
            ],
            responses: { "200": { description: "OK" } },
          },
        },
      },
      components: { schemas: { Removed: { $ref: "#/paths/~1old~1{id}~1{unused}/get/parameters/1/schema" }, Shifted: { $ref: "#/paths/~1old~1{id}~1{unused}/get/parameters/2/schema" } } },
    };
    const next = moveOpenApiOperation(source, "GET:/old/{id}/{unused}", { method: "get", path: "/new/{id}" });
    const components = next.components as OpenApiObject;
    const schemas = components.schemas as OpenApiObject;
    expect(resolveOpenApiRef(next, schemas.Removed)).toEqual({ type: "number", minimum: 1 });
    expect(resolveOpenApiRef(next, schemas.Shifted)).toEqual({ type: "string", format: "uuid" });
    expect(schemas.Removed).toMatchObject({ $ref: expect.stringMatching(/^#\/components\/parameters\//) });
    expect(schemas.Shifted).toMatchObject({ $ref: "#/paths/~1new~1{id}/get/parameters/1/schema" });
  });
});
