# Rust Crates

SpecX uses a small set of Rust crates with one-way dependencies. The names
describe ownership boundaries; a crate is added when it has an independent
consumer or a stable contract, not simply to mirror another repository.

| Crate | Owns | Must not own |
| --- | --- | --- |
| `specx-types` | Shared resource and workspace context structs | Storage, transport, domain workflows |
| `specx-formats` | Pure content transformations and format helpers | Resource lookup, HTTP, database access |
| `specx-platform` | Process paths and shared database URL syntax | SpecX domain rules or database drivers |
| `specx-core` | Catalog invariants, scoped operations, persistence adapters | HTTP framework or frontend state |
| `specx-api` | Routes, request/response DTOs, authorization and storage facade | Host login UI or direct frontend concerns |
| `specx-web` | Executable host, server-side login session, static asset serving | Domain policy and catalog persistence logic |

The current dependency direction is:

```text
apps/desktop --HTTP--> specx-web --> specx-api --> specx-core --> specx-types
apps/desktop --Tauri IPC--> src-tauri --> specx-api
                                                |       |
                                                |       +--> specx-formats
                                                +----------> specx-types
specx-web --> specx-platform
src-tauri --> specx-platform
```

`specx-types`, `specx-formats`, and `specx-platform` do not depend on core, API,
or web crates. PostgreSQL and SQLite remain implementations behind the same
core/API operations. Plugin runtime and AI provider crates are intentionally
deferred until those capabilities have real consumers.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the rationale and crate ownership
rules.
