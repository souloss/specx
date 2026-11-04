# Crate Boundaries

## Design Goal

Shared contracts sit below application logic, hosts adapt runtime concerns, and
dependencies point toward stable lower layers. SpecX crates contain only the
contracts and capabilities needed by this product.

Crates are split where the boundary has an independent consumer or a clear
change cadence. This keeps the workspace extensible without creating empty
framework crates for hypothetical features.

## Ownership

| Crate | Owns | Dependency rule |
| --- | --- | --- |
| `specx-types` | IDs, workspace context, and shared resource facts | Serde only; no runtime, storage, or application dependencies |
| `specx-formats` | Deterministic content normalization and pure format utilities | Standard library only; no resource lookup or I/O |
| `specx-platform` | Environment-derived paths and shared database URL syntax | Standard library only; no business rules or drivers |
| `specx-core` | Catalog invariants and SQLite/PostgreSQL persistence | May depend on types and pure formats; never on HTTP or UI |
| `specx-api` | HTTP routes, API DTOs, authorization checks, storage selection facade | Depends on core; does not choose host login or browser state |
| `specx-web` | Server startup, password session, static files, API mounting | Composes API and platform; does not implement catalog rules |

The dependency graph is intentionally small:

```text
apps/desktop --HTTP--> specx-web --> specx-api --> specx-core --> specx-types
apps/desktop --Tauri IPC--> src-tauri --> specx-api
                                                |       |
                                                |       +--> specx-formats
                                                +----------> specx-types
specx-web --> specx-platform
src-tauri --> specx-platform
```

SQLite and PostgreSQL implement the same store behavior. Hosts choose the
backend from `SPECX_DATABASE_URL` using the shared parser in `specx-platform`;
database access and domain operations remain in the existing persistence and
API layers.

## Hosts and Shared Application

`apps/desktop/` is the Vue/Vite application shared by browser and Tauri.
`crates/specx-web/` is the Rust executable that handles password sessions,
mounts `/api/specx`, and serves the compiled frontend in deployment.
`src-tauri/` is a standalone native shell rather than a business crate. Its IPC
command dispatches into the same
`specx-api` router used by the Web host. Docker builds the frontend first and
copies its static output into the Web host image.

Desktop application updates are another host responsibility: `src-tauri` owns
the official Tauri updater and bounded changelog fetching, while the shared UI
owns update state and confirmation. Versioning and distribution follow
[the desktop release conventions](../docs/guides/desktop-releases.md); no updater
or release-network dependency belongs in the domain crates.

The shared frontend keeps generic controls, appearance, shortcuts, and layout
behavior in `components/ui`, `composables`, `lib`, and `styles`. Catalog-facing
components adapt those primitives through Pinia stores and the backend client.
See [the UI foundation](../docs/architecture/ui-foundation.md) for module
ownership and extension rules.

`specx-types` contains SpecX resource facts. `specx-platform` contains host path
configuration. `specx-formats` provides deterministic content helpers. Add
process management, download, proxy, plugin, or AI infrastructure only when a
concrete SpecX feature has a stable contract and consumer.

## Change Workflow

Add domain invariants to `specx-core` and exercise them with core tests. Keep
wire request/response models and routes in `specx-api`. Keep view state in
`apps/desktop`; catalog data must flow through its HTTP or Tauri transport and
the shared API router. Add a lower-level crate only when its code has a second
consumer or needs an independently testable contract.

The CI checks formatting, frontend type/build, frontend tests, and the Rust
workspace test suite. Use `cargo check --workspace` when validating all feature
and dependency edges without executing tests.

For cross-layer API wire conventions, error behavior, and OpenAPI maintenance,
see [the API contract](../docs/architecture/api-contract.md). For frontend
locale resources and user-visible text, see
[the internationalization conventions](../docs/architecture/i18n.md).
