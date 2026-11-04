# Coding Agent Instructions

Read the relevant project guidance before changing code:

- [Application foundation baseline](docs/architecture/foundation-baseline.md) for current architecture, state ownership, and feature workflow.
- [Code conventions](docs/architecture/code-conventions.md) for naming, layer ownership, DTOs, error handling, and validation expectations.
- [Crate boundaries](crates/ARCHITECTURE.md) before changing Rust dependencies or moving responsibilities between crates.
- [API contract](docs/architecture/api-contract.md) before changing routes, DTOs, status codes, authorization, pagination, or concurrency behavior.
- [Internationalization conventions](docs/architecture/i18n.md) before adding user-visible text or locale behavior.
- [Frontend development guide](apps/desktop/README.md) for Vue, stores, transport, UI, and verification conventions.
- [Desktop release conventions](docs/guides/desktop-releases.md) before changing application versions, release tags, updater behavior, signing, or release notes.

## Change Rules

- Treat executable code, OpenAPI output, and tests as the source of truth for current behavior. Architecture and API documents define boundaries and conventions for new work. Do not describe a planned migration as already implemented.
- Keep domain rules in `core/`, HTTP DTOs and routing in `crates/specx-api/`, host concerns in `crates/specx-web/` or `src-tauri/`, and view state in `apps/desktop/`. Keep dependencies pointing toward stable lower layers.
- Web identity and tenant/actor context must come from the server-side session or a future OIDC integration. `x-specx-*` context headers are an internal host-to-API contract, not credentials; never trust client-supplied tenant or actor headers.
- When changing an API operation, update its OpenAPI annotations and the checked contract fixture in `crates/specx-api/tests/fixtures/openapi-contract.json`. Preserve existing wire behavior unless the change explicitly includes a coordinated migration.
- Define new API or host errors in the shared `crates/specx-api/src/errors.rs` catalog. Return the v1 envelope and update its cross-language fixture; never return Debug text or classify errors by `message`. See [the error contract](docs/architecture/api-error-contract.md).
- Add or change user-visible text in both `zh-CN` and `en`. Use typed keys and named interpolation parameters; keep the locale consistency tests passing. Never use translated text for control flow.
- Update the relevant tests and documentation when a change alters a boundary or externally observable contract. Prefer focused changes over unrelated refactors.

## Verification

Run checks for the changed area, then the relevant repository checks described in `README.md`, `CONTRIBUTING.md`, and the affected package README. At minimum, run `git diff --check` for every change.
