import { describe, expect, it } from "vitest";
import { compareOpenApiDocuments } from "./openapiDiff";

const before = `openapi: 3.0.3
info: { title: API, version: '1' }
paths:
  /users:
    get:
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                type: object
                required: [id]
                properties:
                  id: { type: integer }
`;

describe("OpenAPI semantic diff", () => {
  it("classifies removed operations and added operations", () => {
    const after = `openapi: 3.0.3
info: { title: API, version: '2' }
paths:
  /health:
    get:
      responses: { '200': { description: ok } }
`;
    const result = compareOpenApiDocuments(before, after);
    expect(result.impact).toBe("major");
    expect(result.changes.map((change) => change.message)).toEqual(expect.arrayContaining(["operation_removed", "operation_added"]));
  });

  it("detects response schema removals and treats additive fields as safe", () => {
    const after = before.replace("                  id: { type: integer }", "                  id: { type: integer }\n                  name: { type: string }");
    const result = compareOpenApiDocuments(before, after);
    expect(result.impact).toBe("minor");
    expect(result.changes).toEqual(expect.arrayContaining([expect.objectContaining({ message: "response_property_added", severity: "safe" })]));
  });
});
