# Schema migration policy

`core` owns the schema for both SQLite and PostgreSQL. The adapters expose the
same `schema_migrations` contract and must reach the same latest version before
serving business requests. The current version is **11**.

Every migration is a monotonic integer. A version is applied exactly once and
is recorded only after its DDL and data backfill succeeds. Existing databases
must upgrade through every intermediate marker; a new database must produce
the same final schema. PostgreSQL runs the sequence under a transaction-scoped
advisory lock. SQLite runs with foreign keys and a busy timeout enabled; a
failed startup must return the error and keep the process out of the ready
state.

The initial PostgreSQL schema contains objects introduced by earlier SQLite
migrations, but it still records the intermediate versions (including no-op
markers) so both adapters share one upgrade history. Do not reuse a version,
delete an applied migration, or edit a migration that may already be deployed.
Add the next version and update both adapters in the same change.

Hosts open the adapter before exposing the API router. `/ready` remains false
until the latest marker exists. Web startup reports a failed migration and
does not serve business handlers; the desktop shell remains visible and
retries opening the adapter on the next request. Both hosts therefore follow
this order: install the new binary, acquire the migration lock, migrate, verify
readiness, then expose writes. If migration fails, retain the previous database
and diagnostic error; never continue with partially migrated business handlers.
Release packages must include a backup or provider snapshot policy appropriate
to the deployment.

The API also checks the shared readiness gate before every business request. Only
`/health`, `/ready`, and `/openapi.json` remain available while migrations are
incomplete; other routes return the stable `not_ready` error envelope. Hosts
must surface that state and retry after migration succeeds. The `/ready`
probe refreshes the shared gate after a host recovers its storage connection.

The CI gate runs `pnpm migration:check`, Rust tests for fresh and legacy SQLite
databases, and the PostgreSQL migration tests when a PostgreSQL service is
available. The check rejects missing or divergent versions before a release.
