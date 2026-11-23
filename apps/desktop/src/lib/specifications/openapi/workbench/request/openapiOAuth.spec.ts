import { describe, expect, it } from "vitest";
import { buildOAuthAuthorizationUrl, createOAuthCodeChallenge, createOAuthCodeVerifier, createOAuthState, formEncode, isOAuthTokenExpired, isSafeOAuthEndpoint, parseOAuthTokenResponse } from "./openapiOAuth";

describe("OpenAPI OAuth helpers", () => {
  it("creates a PKCE authorization URL with state and S256 challenge", async () => {
    const verifier = createOAuthCodeVerifier();
    const challenge = await createOAuthCodeChallenge(verifier);
    const state = createOAuthState();
    const url = new URL(
      buildOAuthAuthorizationUrl({
        authorizationUrl: "https://login.example.test/authorize?tenant=demo",
        clientId: "specx-client",
        redirectUri: "http://127.0.0.1:5176/oauth/callback",
        state,
        codeChallenge: challenge,
        scopes: ["read", "write"],
      }),
    );
    expect(url.searchParams.get("tenant")).toBe("demo");
    expect(url.searchParams.get("response_type")).toBe("code");
    expect(url.searchParams.get("client_id")).toBe("specx-client");
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    expect(url.searchParams.get("scope")).toBe("read write");
    expect(url.searchParams.get("state")).toBe(state);
  });

  it("parses token lifetime and keeps refresh tokens available only in memory", () => {
    const token = parseOAuthTokenResponse({ access_token: "access", refresh_token: "refresh", token_type: "Bearer", expires_in: 60 }, 1_000);
    expect(token).toEqual({ accessToken: "access", refreshToken: "refresh", tokenType: "Bearer", expiresAt: 61_000 });
    expect(isOAuthTokenExpired(token, 30_999)).toBe(false);
    expect(isOAuthTokenExpired(token, 31_001)).toBe(true);
    expect(formEncode({ grant_type: "authorization_code", code: "a b", client_secret: undefined })).toBe("grant_type=authorization_code&code=a+b");
  });

  it("rejects token responses without an access token", () => {
    expect(() => parseOAuthTokenResponse({ token_type: "Bearer" })).toThrow("oauth_access_token_missing");
  });

  it("allows HTTPS OAuth endpoints and local loopback HTTP only", () => {
    expect(isSafeOAuthEndpoint("https://login.example.test/authorize")).toBe(true);
    expect(isSafeOAuthEndpoint("http://localhost:8080/authorize")).toBe(true);
    expect(isSafeOAuthEndpoint("http://127.0.0.1:8080/authorize")).toBe(true);
    expect(isSafeOAuthEndpoint("http://[::1]:8080/authorize")).toBe(true);
    expect(isSafeOAuthEndpoint("http://login.example.test/authorize")).toBe(false);
    expect(isSafeOAuthEndpoint("https://user:password@login.example.test/authorize")).toBe(false);
    expect(isSafeOAuthEndpoint("https://login.example.test/authorize#fragment")).toBe(false);
    expect(isSafeOAuthEndpoint("javascript:alert(1)")).toBe(false);
    expect(isSafeOAuthEndpoint("/authorize")).toBe(false);
  });
});
