# Contributing to specX

specX is a focused service directory and specification workspace. Contributions should keep the service catalog, authorization boundary, persistence layer, web host, and Docker deployment understandable as one product.

## Development

Requirements: Node.js 22+, pnpm 10.34.7, Rust 1.90+, and Make.

```sh
pnpm install
pnpm dev
```

Run the API host with a local password when testing persistence:

```sh
SPECX_PASSWORD='choose-a-password-at-least-12-chars' pnpm dev:backend
```

Use `pnpm build:checked` and `pnpm test:specx` before opening a change. Docker changes should also be checked with `pnpm docker:build` when Docker is available.

## Project boundaries

| Path | Purpose |
| --- | --- |
| `apps/desktop/` | Vue 3 + Pinia frontend shared by Web and Tauri |
| `src-tauri/` | Native desktop shell and IPC adapter; intentionally outside `crates/` |
| `core/` | Service, group, tag, specification, authorization, and persistence domain |
| `crates/specx-api/` | HTTP resources and application service boundary |
| `crates/specx-web/` | Password session, request context, static hosting, and API mounting |
| `deploy/` | Docker image and Compose deployment |
| `docs/` | Current architecture and API notes |

The persistence adapters in `core/` are internal specX storage. External administration tools and extension runtimes are outside this repository baseline.

The Web and desktop hosts must call the same `specx-api` router. A transport
change belongs in `apps/desktop/src/lib/backend`; catalog rules belong in
`core/` or `crates/specx-api`, never in a host-specific command or component.

## Change expectations

Keep changes scoped to the layer that owns the behavior. API writes must preserve tenant/workspace authorization and optimistic concurrency. Frontend state should keep the service directory usable when browser storage is unavailable. New deployment settings must use the `SPECX_` prefix and be documented in the Docker deployment notes.

For naming, dependency direction, DTO, error, localization, and testing conventions, follow [the code conventions](docs/architecture/code-conventions.md) and the linked contract guides.

Use clear commit messages such as `feat(api): add service group ordering` or `fix(web): reject invalid session cookies`.
