/** Result of compiling the value entered in an explicit regular-expression search. */
export interface CompiledSearchRegex {
  regex: RegExp | null;
  invalid: boolean;
}

/**
 * Compile the sidebar's explicit regex mode.
 *
 * `/pattern/flags` is accepted for parity with dbx. Plain values use a
 * case-insensitive expression, which keeps the regular search predictable
 * while still allowing callers to opt into other flags with the slash form.
 */
export function compileSearchRegex(query: string): CompiledSearchRegex {
  const value = query.trim();
  if (!value) return { regex: null, invalid: false };

  if (value.startsWith("/")) {
    const lastSlash = value.lastIndexOf("/");
    if (lastSlash > 0) {
      const source = value.slice(1, lastSlash);
      const flags = value.slice(lastSlash + 1);
      try {
        return { regex: new RegExp(source, flags || "i"), invalid: false };
      } catch {
        return { regex: null, invalid: true };
      }
    }
  }

  try {
    return { regex: new RegExp(value, "i"), invalid: false };
  } catch {
    return { regex: null, invalid: true };
  }
}
