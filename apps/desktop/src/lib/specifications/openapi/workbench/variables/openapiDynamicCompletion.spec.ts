import { describe, expect, it, vi } from "vitest";
import { acceptCompletion, closeBrackets, currentCompletions } from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";
import { json } from "@codemirror/lang-json";
import { dynamicCompletionEdit, dynamicCompletionToken, openApiDynamicCompletion } from "./openapiDynamicCompletion";

describe("dynamic variable completion", () => {
  it("completes filtered suggestions when typing into a JSON string with existing closing braces", async () => {
    const prefix = '{"id":"';
    const suffix = '}}","count":1}';
    const editor = new EditorView({
      doc: prefix + suffix,
      selection: { anchor: prefix.length },
      extensions: [json(), closeBrackets(), openApiDynamicCompletion()],
      parent: document.body,
    });
    try {
      editor.focus();
      for (const char of "{{$string.uu") {
        const from = editor.state.selection.main.head;
        const defaultInsert = () => editor.state.update({ changes: { from, insert: char }, selection: { anchor: from + 1 }, userEvent: "input.type" });
        if (!editor.state.facet(EditorView.inputHandler).some((handler) => handler(editor, from, from, char, defaultInsert))) editor.dispatch(defaultInsert());
      }
      await vi.waitFor(() => expect(currentCompletions(editor.state).map(({ label }) => label)).toEqual(["$string.uuid"]));
      expect(acceptCompletion(editor)).toBe(true);
      expect(editor.state.doc.toString()).toBe('{"id":"{{$string.uuid}}","count":1}');
      expect(editor.state.selection.main.head).toBe(prefix.length + "{{$string.uuid}}".length);
    } finally {
      editor.destroy();
    }
  });

  it("replaces a method in place without dropping its arguments or conversion pipeline", () => {
    const before = "value={{%number.in";
    const after = "t(min=7,max=7)|string()}}&other=1";
    const token = dynamicCompletionToken(before);
    expect(token).toEqual({ from: 6, query: "number.in" });
    const edit = dynamicCompletionEdit(token!.from, before.length, after, "{{$number.int}}");
    expect(before.slice(0, edit.from) + edit.insert + (before + after).slice(edit.to)).toBe("value={{$number.int(min=7,max=7)|string()}}&other=1");
    expect(dynamicCompletionToken("{{$number.int(min=")).toBeNull();
  });
});
