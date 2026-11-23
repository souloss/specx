import { autocompletion, ifIn, insertCompletionText, pickedCompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { javascriptLanguage } from "@codemirror/lang-javascript";
import { syntaxTree } from "@codemirror/language";
import { Prec } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { OPENAPI_DYNAMIC_VARIABLES } from "./openapiDynamicVariables";

const methods = OPENAPI_DYNAMIC_VARIABLES.map((item) => ({ ...item, searchName: item.name.toLowerCase() }));
const stringNodes = ["String", "PropertyName", "TemplateString"];

// In strings, keep existing template delimiters instead of adding another pair.
const literalBraces = Prec.high(
  EditorView.inputHandler.of((view, from, to, insert, defaultInsert) => {
    if (insert !== "{" || from !== to || view.composing || view.state.readOnly) return false;
    const node = syntaxTree(view.state).resolveInner(from, -1);
    if (from >= node.to || !stringNodes.includes(node.name)) return false;
    view.dispatch(defaultInsert());
    return true;
  }),
);

export function dynamicCompletionToken(valueBeforeCursor: string) {
  const match = /\{\{[$%]([\w.]*)$/.exec(valueBeforeCursor);
  return match ? { from: match.index, query: match[1]!.toLowerCase() } : null;
}

export function dynamicCompletionOptions(query: string) {
  if (!query) return methods;
  const prefix = [],
    infix = [];
  for (const item of methods) {
    if (item.searchName.startsWith(query)) prefix.push(item);
    else if (item.searchName.includes(query)) infix.push(item);
  }
  return prefix.concat(infix);
}

export function dynamicCompletionEdit(from: number, cursor: number, textAfterCursor: string, expression: string) {
  const methodSuffix = /^[\w.]*/.exec(textAfterCursor)?.[0] ?? "";
  const tail = textAfterCursor.slice(methodSuffix.length);
  const hasModifiers = /^\s*[(|]/.test(tail);
  const braces = hasModifiers ? 0 : (/^\s*\}{1,2}/.exec(tail)?.[0].length ?? 0);
  return { from, to: cursor + methodSuffix.length + braces, insert: hasModifiers ? expression.slice(0, -2) : expression };
}

function source(context: CompletionContext): CompletionResult | null {
  const line = context.state.doc.lineAt(context.pos);
  const token = dynamicCompletionToken(line.text.slice(0, context.pos - line.from));
  if (!token) return null;
  const options = dynamicCompletionOptions(token.query);
  if (!options.length) return null;
  return {
    from: line.from + token.from,
    filter: false,
    options: options.map((item) => ({
      label: `$${item.name}`,
      type: "variable",
      apply(view, completion, from, cursor) {
        const suffix = view.state.sliceDoc(cursor, view.state.doc.lineAt(cursor).to);
        const edit = dynamicCompletionEdit(from, cursor, suffix, item.expression);
        view.dispatch({ ...insertCompletionText(view.state, edit.insert, edit.from, edit.to), annotations: pickedCompletion.of(completion) });
      },
    })),
    update: (_current, _from, _to, next) => source(next),
  };
}

export function openApiDynamicCompletion() {
  return [literalBraces, autocompletion({ override: [source], activateOnTypingDelay: 80, interactionDelay: 0 })];
}

export const openApiScriptDynamicCompletion = [literalBraces, javascriptLanguage.data.of({ autocomplete: ifIn(["String", "TemplateString"], source) }), autocompletion({ activateOnTypingDelay: 80, interactionDelay: 0 })];
