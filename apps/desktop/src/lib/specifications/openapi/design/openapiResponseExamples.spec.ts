import { describe, expect, it } from "vitest";
import { applyResponseExampleEdits, parseResponseExample, responseCaptureTarget } from "./openapiResponseExamples";

describe("OpenAPI response examples", () => {
  it("targets the actual status and media without replacing a shared response or declared schema", () => {
    const responses = {
      "200": { content: { "Application/JSON": { schema: false, examples: { original: { value: false } } }, "text/plain": {} } },
      "418": { $ref: "#/components/responses/Problem" },
      "4XX": { $ref: "#/components/responses/Problem" },
    };
    expect(responseCaptureTarget(responses, "422", "Application/Problem+JSON; charset=UTF-8")).toEqual({ status: "422", mediaType: "application/problem+json", action: "newResponse", canInfer: true, names: [] });
    expect(responseCaptureTarget(responses, "200", "application/json")).toMatchObject({ mediaType: "Application/JSON", action: "existing", canInfer: false, names: ["original"] });
    expect(responseCaptureTarget(responses, "200", "text/plain")).toMatchObject({ action: "existing", canInfer: true });
    expect(responseCaptureTarget(responses, "200", "text/csv")).toMatchObject({ action: "newMedia", canInfer: true });
    expect(responseCaptureTarget(responses, "418", "application/problem+json")).toBeNull();
    expect(responseCaptureTarget(responses, "205", "text/plain")).toBeNull();
  });
  it("keeps a reviewed JSON value or rejects values that the specification serializer cannot retain", () => {
    const mediaType = "application/problem+json";
    expect(parseResponseExample('{"a":1.25e2,"b":0.10,"text":"-0","array":[{"a":3}]}', mediaType)).toEqual({ value: { a: 125, b: 0.1, text: "-0", array: [{ a: 3 }] } });
    for (const input of ['{"a":1,"\\u0061":2}', "[9007199254740993]", "1.0000000000000001", "1e400", "1e-400", "-0", "[".repeat(65) + "0" + "]".repeat(65)]) {
      expect(parseResponseExample(input, mediaType)).toEqual({ error: "lossyJson" });
    }
    expect(parseResponseExample("{", mediaType)).toEqual({ error: "invalidJson" });
    expect(parseResponseExample("中文".repeat(6000), "text/plain")).toEqual({ error: "tooLarge" });
    expect(parseResponseExample("  null\n", "text/plain")).toEqual({ value: "  null\n" });
  });

  it("adds and updates named examples without dropping the existing value, metadata, or reference", () => {
    const schema = { $ref: "#/components/schemas/Pet" };
    const media = { schema, example: null, examples: { "default-example": { $ref: "#/components/examples/Pet" }, note: { summary: "Note", value: 1 }, remove: { value: true } } };
    expect(applyResponseExampleEdits(media, "application/json", { note: "2", remove: null, newExample: "[3]" })).toEqual({
      schema,
      examples: { "default-example": media.examples["default-example"], "default-example-2": { value: null }, note: { summary: "Note", value: 2 }, newExample: { value: [3] } },
    });
    expect(media.example).toBeNull();
    expect(media.examples.note.value).toBe(1);
  });
});
