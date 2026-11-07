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
| `specx-controller` | Generic controller runtime, reconciliation queue, platform ContentStore, Git Provider and Markdown controller adapter | HTTP routes, authorization, direct database access and Core persistence |
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
specx-controller --> specx-types
specx-controller --> specx-formats
```

`specx-types`, `specx-formats`, and `specx-platform` do not depend on core, API,
or web crates. PostgreSQL and SQLite remain implementations behind the same
core/API operations. `specx-controller` is the shared orchestration foundation
and currently carries the first Markdown vertical slice without adding
persistence or HTTP knowledge to the runtime. Plugin packages and AI provider
crates remain deferred until those capabilities have real consumers.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the rationale and crate ownership
rules.
