import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";

const MAX_PRETTY_LENGTH = 2 * 1024 * 1024;
const MAX_PRETTY_DEPTH = 64;
const jsonTokens = /"(?:[^"\\]|\\.)*"|[{}[\],:]|[^\s{}[\],:"]+/gu;

/** Validate JSON, then change whitespace only. Parsing and reserializing would round large numbers and discard duplicate keys. */
export function formatResponseJson(source: string): { text: string; formatted: boolean } | null {
  try {
    JSON.parse(source);
  } catch {
    return null;
  }

  const parts: string[] = [];
  let depth = 0;
  let length = 0;
  let previous = "";
  for (const [token] of source.matchAll(jsonTokens)) {
    const opens = token === "{" || token === "[";
    const closes = token === "}" || token === "]";
    const afterOpen = previous === "{" || previous === "[";
    if (closes) depth -= 1;
    const prefix = previous === ":" ? " " : previous === "," || (afterOpen && !closes) || (closes && !afterOpen) ? "\n" + "  ".repeat(depth) : "";
    if (opens) depth += 1;
    length += prefix.length + token.length;
    if (length > MAX_PRETTY_LENGTH || depth > MAX_PRETTY_DEPTH) return { text: source, formatted: false };
    parts.push(prefix + token);
    previous = token;
  }
  return { text: parts.join(""), formatted: true };
}

export type ResponseMatch = { from: number; to: number };

/** Keep match offsets in a compact index. Literal Unicode matching keeps offsets in the original text. */
export function findResponseMatches(text: string, query: string): Uint32Array {
  if (!query) return new Uint32Array();
  const literal = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const offsets: number[] = [];
  for (const match of text.matchAll(new RegExp(literal, "giu"))) offsets.push(match.index, match.index + match[0].length);
  return Uint32Array.from(offsets);
}

export function responseMatchAt(index: Uint32Array, position: number): ResponseMatch | null {
  const offset = position * 2;
  return offset >= 0 && offset < index.length ? { from: index[offset]!, to: index[offset + 1]! } : null;
}

/** Binary search for the first match ending after the visible range starts. */
export function firstVisibleResponseMatch(index: Uint32Array, from: number): number {
  let low = 0;
  let high = index.length / 2;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (index[mid * 2 + 1]! <= from) low = mid + 1;
    else high = mid;
  }
  return low;
}

const jsonValues = new Set(["Object", "Array", "String", "Number", "True", "False", "Null"]);

export function selectedResponseJson(state: EditorState): (ResponseMatch & { path: string }) | null {
  const head = state.selection.main.from;
  const tree = syntaxTree(state);
  if (head > tree.length) return null;
  let node = tree.resolveInner(head, 1);
  while (!jsonValues.has(node.name)) {
    if (node.name === "Property") {
      if (!node.lastChild || !jsonValues.has(node.lastChild.name)) return null;
      node = node.lastChild;
      break;
    }
    if (node.name === "JsonText") {
      if (!node.firstChild || !jsonValues.has(node.firstChild.name)) return null;
      node = node.firstChild;
      break;
    }
    if (!node.parent) return null;
    node = node.parent;
  }

  const { from, to } = node;
  // The parser may have only processed the viewport. Never offer a partial object for copying.
  if (node.name === "Object" || node.name === "Array") {
    const close = node.name === "Object" ? "}" : "]";
    if (node.lastChild?.name !== close) return null;
  }
  const segments: string[] = [];
  for (let child = node, parent = node.parent; parent; child = parent, parent = parent.parent) {
    if (parent.name === "Property") {
      const name = parent.getChild("PropertyName");
      if (!name) return null;
      const key: string = JSON.parse(state.sliceDoc(name.from, name.to));
      segments.push(/^[A-Za-z_$][\w$]*$/.test(key) ? `.${key}` : `[${JSON.stringify(key)}]`);
    } else if (parent.name === "Array") {
      let index = 0;
      for (let sibling = child.prevSibling; sibling; sibling = sibling.prevSibling) {
        if (jsonValues.has(sibling.name)) index += 1;
      }
      segments.push(`[${index}]`);
    }
  }
  return { from, to, path: "$" + segments.reverse().join("") };
}
