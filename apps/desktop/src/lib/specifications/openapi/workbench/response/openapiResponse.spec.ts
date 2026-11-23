import { EditorState } from "@codemirror/state";
import { json } from "@codemirror/lang-json";
import { describe, expect, it } from "vitest";
import { findResponseMatches, firstVisibleResponseMatch, formatResponseJson, responseMatchAt, selectedResponseJson } from "./openapiResponse";

describe("response JSON formatting and inspection", () => {
  it("changes whitespace without rounding numbers, reordering keys, or discarding duplicates", () => {
    const raw = String.raw` { "4": 9007199254740993, "2":-0, "4":1E+400,"text":"a, [b] \" \u0041", "empty":[{},[]] } `;
    const formatted = formatResponseJson(raw)!;
    expect(formatted.formatted).toBe(true);
    expect(formatted.text).toBe(String.raw`{
  "4": 9007199254740993,
  "2": -0,
  "4": 1E+400,
  "text": "a, [b] \" \u0041",
  "empty": [
    {},
    []
  ]
}`);
  });

  it("leaves malformed text and deeply nested documents available in their original form", () => {
    expect(formatResponseJson('{"incomplete":[')).toBeNull();
    expect(formatResponseJson("null")).toEqual({ text: "null", formatted: true });
    const deep = "[".repeat(90) + "0" + "]".repeat(90);
    expect(formatResponseJson(deep)).toEqual({ text: deep, formatted: false });
  });

  it("copies JSONPath and exact source values from nested fields and array entries", () => {
    const source = '{"items":[null,{"a.b":9007199254740993,"a\\\"b":"text"}]}';
    const inspect = (needle: string) => {
      const state = EditorState.create({ doc: source, extensions: [json()], selection: { anchor: source.indexOf(needle) + 1 } });
      const selected = selectedResponseJson(state)!;
      return { path: selected.path, value: source.slice(selected.from, selected.to) };
    };
    expect(inspect("a.b")).toEqual({ path: '$.items[1]["a.b"]', value: "9007199254740993" });
    expect(inspect("9007199")).toEqual({ path: '$.items[1]["a.b"]', value: "9007199254740993" });
    expect(inspect("null")).toEqual({ path: "$.items[0]", value: "null" });
    expect(inspect("text")).toEqual({ path: '$.items[1]["a\\"b"]', value: '"text"' });
  });

  it("uses literal case insensitive matches with offsets in the source", () => {
    const matches = findResponseMatches("İ + A.b a.B [x]", "a.b");
    expect(matches.length).toBe(4);
    expect(responseMatchAt(matches, 0)).toEqual({ from: 4, to: 7 });
    expect(responseMatchAt(matches, 1)).toEqual({ from: 8, to: 11 });
    expect(responseMatchAt(matches, 2)).toBeNull();
    expect(firstVisibleResponseMatch(matches, 7)).toBe(1);
    expect(findResponseMatches("ab [x] z", "[x]").length).toBe(2);
  });
});
