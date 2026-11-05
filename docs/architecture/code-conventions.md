# Code Conventions

This guide records the conventions for new and changed code. Follow the existing
local pattern when it is more specific. Keep the dependency direction explicit:
shared contracts and domain rules sit below hosts and UI.

## Ownership and Dependencies

| Location | Owns | Must not own |
| --- | --- | --- |
| `crates/specx-types` | Shared IDs, context, and resource facts | I/O, host state, UI behavior |
| `crates/specx-formats` | Deterministic, pure content transforms | Resource lookup, persistence, HTTP |
| `core` | Domain invariants and persistence adapters | HTTP DTOs, locale keys, Vue behavior |
| `crates/specx-api` | Routes, DTOs, authorization, error catalog, OpenAPI | Login/session implementation, frontend state |
| `crates/specx-web`, `src-tauri` | Host lifecycle and transport adaptation | Duplicate domain or API behavior |
| `apps/desktop/src/stores` | Shared client state and user commands | Direct database or host-specific business calls |
| `apps/desktop/src/components` | Rendering and interaction dispatch | Persistence rules or duplicated API mapping |
| `apps/desktop/src/lib/backend` | HTTP/Tauri transport and wire parsing | Feature-specific view state |

Dependencies point inward. Add a module or crate when it gives behavior a clear
owner or independent consumer, not just to make the directory tree deeper.

## Rust

- Use `rustfmt` formatting. Modules, functions, methods, and fields use
  `snake_case`; types, traits, and enum variants use `UpperCamelCase`; constants
  use `SCREAMING_SNAKE_CASE`.
- Keep `cargo clippy --all-targets -- -D warnings` clean for changed workspace
  crates. Avoid `unsafe`; when an unsafe boundary is necessary, keep it small
  and document its invariants at the call site.
- Keep domain failures as typed errors. Map them at the API boundary; do not
  return formatted `Debug` output or storage internals to clients.
- Keep API DTO serialization explicit with `#[serde(rename_all = "camelCase")]`
  where it is the wire convention. Do not serialize persistence/domain structs
  directly as an incidental API contract.
- Propagate expected failures with `Result`. Use `expect` only for a stated
  invariant or a value validated immediately beforehand; do not use panics as
  ordinary error handling.
- Keep async boundaries at I/O and host edges. Pure domain and format code
  should remain synchronous and independently testable.

## Vue and TypeScript

- Use `oxfmt` formatting and the repository's strict TypeScript checks. Variables,
  functions, and object properties use `camelCase`; types and Vue components use
  `UpperCamelCase`. Follow local file naming: Vue components are PascalCase,
  utilities and stores are camelCase.
- Run `pnpm --dir apps/desktop lint` for the Oxlint correctness rules. Keep
  intentional exceptions local and documented with an inline disable; do not
  weaken the project configuration to hide an unrelated warning.
- Keep `vue-tsc --noEmit` clean under strict mode. Avoid `any`; use `unknown`
  at untrusted boundaries and narrow it before use.
- Keep components focused on rendering and dispatch. Put shared state and
  commands in Pinia stores, reusable deterministic logic in `lib/`, and host
  differences behind `lib/backend` transports.
- Represent API data with explicit TypeScript types. Keep wire DTO names aligned
  with the backend's camelCase JSON; do not silently coerce malformed responses
  into valid domain state.
- Put user-visible text in both locale resources and access it through typed
  translation keys. Never use translated text as a program condition.
- Prefer small named functions and explicit types at cross-layer boundaries.
  Avoid broad `any`, hidden mutation, duplicate transport logic, and abstractions
  that have only one trivial caller.

## API and Errors

- Define request and response DTOs beside their owning API resource. Use explicit
  validation, authorization, and concurrency checks at the API boundary; keep
  domain invariants in `core`.
- JSON uses camelCase. Keep route names resource-oriented and update the
  OpenAPI annotations and checked fixture when an operation changes.
- Return errors through the shared v1 catalog. Error codes are unique stable
  lower-case `snake_case` identities; status expresses the HTTP result, and
  `messageKey/messageParams` provide localized summaries. Never add a parallel
  `{code,message}` response, host-specific mapping, or client compatibility
  adapter. See [the error contract](api-error-contract.md).
- Treat authentication and workspace context as host-owned and server-verified.
  Do not trust client-supplied actor or tenant identity.
- Schema changes use the next monotonic migration version in both persistence
  adapters. Run `pnpm migration:check` and follow the startup/update rules in
  [the migration policy](storage-migrations.md); never edit an applied version.

## Tests and Documentation

- Test observable behavior and boundary contracts, not a line-for-line copy of
  implementation. Keep pure domain tests independent of HTTP; cover API status,
  authorization, response shape, and concurrency at the API boundary.
- For cross-layer changes, update all affected transports, locale files,
  fixtures, and architecture documentation in the same change.
- A boundary rule is complete only when executable behavior, OpenAPI output,
  checked fixtures, and focused tests agree. Treat a document-only rule as
  incomplete until a formatter, type check, lint rule, contract test, or
  migration check enforces it.
- CI keeps frontend, Core/API/SQLite, PostgreSQL integration, desktop/Tauri,
  and release-contract checks as separate required jobs. PostgreSQL integration
  runs with `SPECX_REQUIRE_POSTGRES=1`; a missing test URL is therefore a
  failure in that job instead of a skipped test.
- Run the focused package checks first. Before finishing, run
  `git diff --check`, frontend formatting/type/tests for frontend changes, and
  `cargo fmt --all -- --check` plus the affected Rust checks/tests for Rust
  changes. Use `pnpm test:specx` for a full repository verification when the
  change crosses major boundaries.

The repository agent entry point is [AGENTS.md](../../AGENTS.md). The API,
internationalization, and frontend-specific guides remain authoritative for
their respective contracts.
