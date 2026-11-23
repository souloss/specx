export type OAuthFlow = {
  authorizationUrl?: string;
  tokenUrl?: string;
};

export type OAuthToken = {
  accessToken: string;
  refreshToken?: string;
  tokenType: string;
  expiresAt?: number;
};

export type OAuthTokenResponse = {
  access_token?: unknown;
  refresh_token?: unknown;
  token_type?: unknown;
  expires_in?: unknown;
};

export function isSafeOAuthEndpoint(value: string): boolean {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return false;
  }
  if (url.username || url.password || url.hash) return false;
  if (url.protocol === "https:") return true;
  if (url.protocol !== "http:") return false;
  return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(url.hostname.toLowerCase());
}

function randomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length);
  if (!globalThis.crypto?.getRandomValues) throw new Error("oauth_crypto_unavailable");
  globalThis.crypto.getRandomValues(bytes);
  return bytes;
}

function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}

export function createOAuthState(): string {
  return base64UrlEncode(randomBytes(32));
}

export function createOAuthCodeVerifier(): string {
  return base64UrlEncode(randomBytes(48));
}

export async function createOAuthCodeChallenge(verifier: string): Promise<string> {
  if (!globalThis.crypto?.subtle) throw new Error("oauth_crypto_unavailable");
  const digest = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64UrlEncode(new Uint8Array(digest));
}

export function buildOAuthAuthorizationUrl(input: { authorizationUrl: string; clientId: string; redirectUri: string; state: string; codeChallenge: string; scopes: string[] }): string {
  const url = new URL(input.authorizationUrl);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", input.clientId);
  url.searchParams.set("redirect_uri", input.redirectUri);
  url.searchParams.set("state", input.state);
  url.searchParams.set("code_challenge", input.codeChallenge);
  url.searchParams.set("code_challenge_method", "S256");
  if (input.scopes.length) url.searchParams.set("scope", input.scopes.join(" "));
  return url.toString();
}

export function parseOAuthTokenResponse(value: unknown, now = Date.now()): OAuthToken {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("oauth_token_response_invalid");
  const response = value as OAuthTokenResponse;
  if (typeof response.access_token !== "string" || !response.access_token.trim()) throw new Error("oauth_access_token_missing");
  const expiresIn = typeof response.expires_in === "number" && Number.isFinite(response.expires_in) && response.expires_in > 0 ? response.expires_in : undefined;
  return {
    accessToken: response.access_token,
    refreshToken: typeof response.refresh_token === "string" && response.refresh_token ? response.refresh_token : undefined,
    tokenType: typeof response.token_type === "string" && response.token_type ? response.token_type : "Bearer",
    expiresAt: expiresIn ? now + expiresIn * 1000 : undefined,
  };
}

export function isOAuthTokenExpired(token: OAuthToken, now = Date.now()): boolean {
  return token.expiresAt !== undefined && token.expiresAt <= now + 30_000;
}

export function formEncode(values: Record<string, string | undefined>): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(values)) if (value !== undefined) params.set(key, value);
  return params.toString();
}
