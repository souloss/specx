// @vitest-environment node
import { loadJq, type Jq } from "jq-wasm";
import { beforeAll, describe, expect, it } from "vitest";
import { evaluateResponseFilter } from "./openapiResponseFilterEngine";

let jq: Jq;
beforeAll(async () => {
  jq = await loadJq();
});

describe("response filtering with jq", () => {
  it("runs Hoppscotch-style filters against the original JSON without rounding identifiers", () => {
    const body = '{"data":[{"id":9007199254740993,"name":"a\\nb"},{"id":2,"name":"c","skip":true},{"id":3,"name":"d"}]}';
    const result = evaluateResponseFilter(jq, body, ".data[] | select(.skip != true) | {id,name}");
    expect(result).toEqual({ count: 2, text: '[\n  {\n    "id": 9007199254740993,\n    "name": "a\\nb"\n  },\n  {\n    "id": 3,\n    "name": "d"\n  }\n]' });
    expect(evaluateResponseFilter(jq, "null", ".")).toEqual({ count: 1, text: "null" });
    expect(evaluateResponseFilter(jq, "null", "empty")).toEqual({ count: 0, text: "" });
  });

  it("rejects oversized and failed results without returning a partial value or raw runtime error", () => {
    expect(evaluateResponseFilter(jq, "null", "range(0; 1001)")).toEqual({ error: "limit" });
    expect(evaluateResponseFilter(jq, "null", '"中" * 350000')).toEqual({ error: "limit" });
    expect(evaluateResponseFilter(jq, '"private response"', "(0, error(.))")).toEqual({ error: "evaluation" });
    expect(evaluateResponseFilter(jq, "null", ".[")).toMatchObject({ error: "syntax" });
    expect(evaluateResponseFilter(jq, "null", "-n")).toMatchObject({ error: "syntax" });
  });
});
