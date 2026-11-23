# API Contract and Implementation Conventions

This document separates the current wire contract from conventions for new work. The checked-in OpenAPI document and its contract fixture are authoritative for operation-specific paths, fields, headers, and status codes. This guide describes cross-cutting rules and must be updated when those rules change.

## Current Boundary

- The HTTP API is mounted by `specx-web` at `/api/specx`; OpenAPI is served at `/api/specx/openapi.json`.
- The same `specx-api` router serves HTTP and Tauri IPC. Host code authenticates the caller and supplies request context; resource authorization and domain work remain in the API and Core layers.
- The Web host derives actor and tenant identity from its server-side `specx_session` cookie. The requested workspace is selected per request and membership is checked by the API. Tauri supplies its local actor through the native host.
- `x-specx-tenant-id`, `x-specx-workspace-id`, and `x-specx-actor-id` are internal host-to-API context headers. They are not authentication. Public deployments must not trust tenant or actor identity supplied directly by a client.
- Resource routes, DTOs, and OpenAPI annotations currently live mainly in `crates/specx-api/src/resources.rs`; this file is large, but new modules should be introduced when ownership or independent testing benefits, not only to create more directories.

The authenticated workbench also exposes the bounded request Agent at
`POST /services/{service_id}/openapi/agent/requests/{request_id}` and idempotent
`DELETE` cancellation. Editor-or-higher service access is required; active requests
are scoped to tenant/workspace/actor/service/id. The endpoint returns raw body bytes
as base64, duplicate-preserving header lists, connection timings and byte counts.
`network.tls` carries the final connection's negotiated protocol/cipher and leaf
certificate CN, issuer CN and UTC expiry (nullable for plaintext or unsupported
native backends). `requestMethod` and `requestBodyBase64`, together with `url`
and `requestHeaders`, describe the final request after redirect rewrites.
`disabledHeaders` optionally suppresses named headers after body serialization, including native Agent defaults. Header names are validated and bounded.
It does not store target credentials or payloads. Public-only target policy and
explicit host origin grants are documented in [the workbench guide](../guides/openapi-workbench.md).
Agent 502 errors distinguish `agent_resolve_failed`, `agent_connection_failed`,
`agent_response_read_failed`, `agent_response_decoding_failed` and
`agent_response_headers_too_large`, alongside existing TLS and redirect errors.
The server logs only the typed failure category, without target URLs or request data.

## Wire Conventions

- JSON field names use `camelCase`; Rust request and response DTOs stay in `specx-api`, while shared, transport-neutral facts belong in `specx-types`.
- Keep request DTOs explicit and validate values at the API boundary. Use `deny_unknown_fields` for request structs where compatibility permits, and test rejection behavior when strictness is part of the contract.
- Resource collections use stable ordering. Existing group, tag, specification, revision, member, and service list APIs accept `limit` and `offset` with a default of 50 and a maximum of 200. Service listing also supports a filter-bound `cursor`; it returns both `nextOffset` and `nextCursor`. Do not assume every list operation is paginated; follow its OpenAPI schema.
- Existing resource creation operations commonly return `200`; invitations use `201`, and deletes commonly return `204`. Preserve each operation's current documented status. For new operations, choose and document status codes deliberately, and cover them in OpenAPI and tests.
- OpenAPI provider routes live under `/services/{serviceId}/openapi`. Provider settings and environment CRUD are workspace-scoped. Each environment has a default `baseUrl` and optional named `servers` with stable IDs and independent proxy modes. An update that omits `servers` preserves the list. An empty list clears it. `POST /services/{serviceId}/openapi/environments/{environmentId}/resolve` returns enabled resolved variables and configured addresses for an explicitly started request, with `Cache-Control: no-store`. Sensitive values are never returned by provider or environment list responses.
- `POST /services/{serviceId}/openapi/mock` creates or rotates a service-scoped public mock token and returns it once; provider settings expose only `mockEnabled`. The database stores only the SHA-256 token hash. `DELETE` revokes the token. Public `GET /mock/openapi/{token}/{path}` resolves a matching OpenAPI operation and returns its explicit example or a bounded schema-generated response; unknown tokens and paths return `404`.
- OpenAPI publication management requires workspace owner/admin membership. `GET /specifications/{specificationId}/publication-preview/{version}` returns the sanitized saved revision and its SHA-256 `digest`; `POST /specifications/{specificationId}/publications` accepts `{version, title, description, theme, showDownload, allowTryIt, digest, environments?}`, rejects an unreviewed digest with `412`, and creates a random share ID with `201`. One active share is allowed per revision (`409` on duplicates). Listing uses offset pagination; `DELETE /specifications/{specificationId}/publications/{publicationId}` revokes with `204`. Publication and revocation are audited atomically without changing the specification ETag or revision. `theme` is `light` or `dark`; `description` is limited to 500 characters, `showDownload` controls the public definition download action, and `allowTryIt` controls the anonymous request workbench.
- Anonymous `GET /public/openapi/{publicationId}` returns only `{publication, content}` from that immutable snapshot; unknown or revoked IDs return `404`. Snapshots strip source root/path/operation runtime servers, including callback paths, and source `x-specx-public-environments`. They never resolve provider environments. Optional explicitly reviewed `environments` contain only `{name, baseUrl, variables: [{key, value}]}` and are validated (20 environments, 100 variables per environment, unique names/keys, absolute expanded HTTP(S) URLs without credentials/query/fragment). These are stored in the immutable snapshot as `x-specx-public-environments` and resolved standard `servers`. The preview digest still covers the sanitized saved revision before this explicit configuration is added; the final configured snapshot is checked for known workspace Secret values. Known workspace Secret values in the document or title cause publication to fail with `422`; owners must also review examples and other content. The Web reader at `/docs/openapi/{publicationId}` and the random publication subdomain `<publicationId>.<public-origin>/` use no workspace APIs. If `allowTryIt` is enabled, the public reader may send browser requests to an explicit absolute HTTP(S) URL supplied by the reader or selected from these public configurations with `credentials: omit`; this never resolves workspace environments or persists workspace history. Their HTML disables referrers, indexing and framing; API responses use `Cache-Control: no-store`. Production deployments must route wildcard DNS and a reverse proxy for the public origin to the Web host. The Web host strips client-supplied tenant/actor headers before deriving authenticated context.
- `Idempotency-Key` is supported on selected create operations. Retrying the same logical operation must reuse the same key and request. Reusing a key with a different request conflicts. Check the operation's OpenAPI definition before assuming support.
- Service and specification representations use strong ETags for applicable writes. Send the current ETag in `If-Match`; stale writes fail with `412`, and operations that require a missing precondition fail with `428`. Check each operation because coverage is resource-specific.
- Timestamps are serialized as strings. Preserve the existing representation and test its schema when adding or changing timestamp fields.

## Status Codes

The table summarizes current API-generated errors; it is not a claim that every operation returns every status. Use the operation's OpenAPI responses for its exact behavior.

| Status | Current use |
| --- | --- |
| `400` | Invalid request syntax, query, path, pagination, or filter values |
| `401` | Missing API request context; Web login also uses `invalid_credentials` |
| `403` | Failed membership/authorization or rejected cross-origin write |
| `404` | Resource not found (some scoped lookup failures may intentionally be `403`) |
| `405` | Unsupported HTTP method (preserves `Allow`) |
| `409` | Domain conflict, last-owner protection, duplicate, or idempotency-key mismatch |
| `412` | `If-Match` does not match the current representation |
| `413` | Request body exceeds the supported size |
| `415` | Unsupported request content type |
| `422` | Invalid request body or domain validation |
| `428` | Required `If-Match` is missing |
| `429` | Request rate limit exceeded |
| `500` | Storage or internal operation failed |
| `503` | Storage is not ready |

HTTP status communicates the protocol outcome; the error `code` is the stable machine identity. Callers should branch on status and code, never on localized text.

## Error Response

API and Web login failures use the shared v1 envelope. The desktop transport preserves the same shape:

```json
{
  "version": 1,
  "code": "precondition_failed",
  "messageKey": "apiError.preconditionFailed",
  "messageParams": {}
}
```

`version` identifies the error-envelope schema, not the API or application release. `code` is a stable, unique identity for one distinguishable error; HTTP status remains the transport result. `messageKey` and `messageParams` let each client render the same error in its active locale. The response contains no localized or diagnostic prose.

`crates/specx-api/src/errors.rs` owns the catalog and serialization. Core variants map exhaustively to safe error identities; framework rejections also use the envelope. The shared frontend accepts only version `1`, validates the code shape, key, and parameters before localization, retains machine identity, and displays a generic localized summary for unknown or malformed errors.

Use unique, stable, specific `snake_case` codes for new errors and preserve their meaning after release. Never expose credentials, request bodies, SQL, stack traces, Rust Debug output, or internal session data. See [the error contract](api-error-contract.md) for field rules and the completion checklist for a new error.

## Contract Maintenance

- `crates/specx-api/src/resources.rs` defines routes, DTO serialization, and operation-level OpenAPI metadata. `crates/specx-api/src/lib.rs` maps API errors into the shared catalog in `errors.rs`.
- `crates/specx-api/tests/fixtures/openapi-contract.json` is a checked contract snapshot. When an operation changes, update its route, method, response status, parameters, headers, or schema entries as applicable, then run the OpenAPI contract test.
- Update `crates/specx-api/tests/fixtures/error-catalog.json` when the catalog changes. Rust checks it against the actual catalog, and frontend tests verify every entry against both locales.
- Add API tests for authorization scope, success shape, relevant failure status/code, and concurrency/idempotency behavior. Keep Core tests focused on domain invariants rather than HTTP details.
- When changing the envelope, update HTTP, Web authentication, Tauri, OpenAPI, catalog fixtures, and both locales in one change. Update this guide when a cross-cutting convention changes.
