export type ResponseCookie = { name: string; value: string; domain: string; path: string; expires: string; httpOnly: boolean; secure: boolean; sameSite: string };
/** Set-Cookie is not comma-separated: an Expires attribute can contain a comma. */
export function parseResponseCookies(headers: Array<[string, string]>, url: string): ResponseCookie[] {
  let host = "";
  let defaultPath = "/";
  try {
    const parsed = new URL(url);
    host = parsed.hostname;
    const pathname = parsed.pathname;
    if (pathname.startsWith("/") && pathname.lastIndexOf("/") > 0) defaultPath = pathname.slice(0, pathname.lastIndexOf("/"));
  } catch {
    /* Empty response target. */
  }
  return headers
    .filter(([name]) => name.toLowerCase() === "set-cookie")
    .flatMap(([, header]) => {
      const [first, ...attributes] = header.split(";");
      const at = first?.indexOf("=") ?? -1;
      if (!first || at <= 0) return [];
      const cookie: ResponseCookie = { name: first.slice(0, at).trim(), value: first.slice(at + 1), domain: host, path: defaultPath, expires: "", httpOnly: false, secure: false, sameSite: "" };
      let maxAge: string | undefined;
      for (const part of attributes) {
        const [key, ...rest] = part.trim().split("=");
        const value = rest.join("=");
        switch (key?.toLowerCase()) {
          case "domain":
            cookie.domain = value;
            break;
          case "path":
            cookie.path = value;
            break;
          case "expires":
            cookie.expires = value;
            break;
          case "max-age":
            maxAge = value;
            break;
          case "httponly":
            cookie.httpOnly = true;
            break;
          case "secure":
            cookie.secure = true;
            break;
          case "samesite":
            cookie.sameSite = value;
            break;
        }
      }
      if (maxAge !== undefined) cookie.expires = `Max-Age=${maxAge}`;
      return [cookie];
    });
}
