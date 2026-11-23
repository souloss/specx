import { describe, expect, it } from "vitest";
import { parseResponseCookies } from "./openapiResponseCookies";

describe("response cookie parsing", () => {
  it("keeps Set-Cookie headers separate and preserves Expires commas", () => {
    expect(
      parseResponseCookies(
        [
          ["set-cookie", "session=a=b; Expires=Wed, 21 Oct 2030 07:28:00 GMT; HttpOnly"],
          ["Set-Cookie", "theme=dark; Secure; SameSite=Lax"],
        ],
        "https://api.example.test/v1/pets/42",
      ),
    ).toEqual([
      { name: "session", value: "a=b", domain: "api.example.test", path: "/v1/pets", expires: "Wed, 21 Oct 2030 07:28:00 GMT", httpOnly: true, secure: false, sameSite: "" },
      { name: "theme", value: "dark", domain: "api.example.test", path: "/v1/pets", expires: "", httpOnly: false, secure: true, sameSite: "Lax" },
    ]);
  });
  it("uses Max-Age over Expires regardless of attribute order", () => {
    expect(parseResponseCookies([["set-cookie", "a=b; Max-Age=60; Expires=Wed, 21 Oct 2030 07:28:00 GMT"]], "https://api.example.test/pets")[0]?.expires).toBe("Max-Age=60");
  });
});
