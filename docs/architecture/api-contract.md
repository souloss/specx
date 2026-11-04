# API Contract and Implementation Conventions

This document separates the current wire contract from conventions for new work. The checked-in OpenAPI document and its contract fixture are authoritative for operation-specific paths, fields, headers, and status codes. This guide describes cross-cutting rules and must be updated when those rules change.

## Current Boundary

- The HTTP API is mounted by `specx-web` at `/api/specx`; OpenAPI is served at `/api/specx/openapi.json`.
- The same `specx-api` router serves HTTP and Tauri IPC. Host code authenticates the caller and supplies request context; resource authorization and domain work remain in the API and Core layers.
- The Web host derives actor and tenant identity from its server-side `specx_session` cookie. The requested workspace is selected per request and membership is checked by the API. Tauri supplies its local actor through the native host.
- `x-specx-tenant-id`, `x-specx-workspace-id`, and `x-specx-actor-id` are internal host-to-API context headers. They are not authentication. Public deployments must not trust tenant or actor identity supplied directly by a client.
- Resource routes, DTOs, and OpenAPI annotations currently live mainly in `crates/specx-api/src/resources.rs`; this file is large, but new modules should be introduced when ownership or independent testing benefits, not only to create more directories.

## Wire Conventions

- JSON field names use `camelCase`; Rust request and response DTOs stay in `specx-api`, while shared, transport-neutral facts belong in `specx-types`.
- Keep request DTOs explicit and validate values at the API boundary. Use `deny_unknown_fields` for request structs where compatibility permits, and test rejection behavior when strictness is part of the contract.
- Resource collections use stable ordering. Existing group, tag, specification, revision, member, and service list APIs accept `limit` and `offset` with a default of 50 and a maximum of 200. Service listing also supports a filter-bound `cursor`; it returns both `nextOffset` and `nextCursor`. Do not assume every list operation is paginated; follow its OpenAPI schema.
- Existing resource creation operations commonly return `200`; invitations use `201`, and deletes commonly return `204`. Preserve each operation's current documented status. For new operations, choose and document status codes deliberately, and cover them in OpenAPI and tests.
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
