# Schema migration policy

`core` owns the schema for both SQLite and PostgreSQL. The adapters expose the
same `schema_migrations` contract and must reach the same latest version before
serving business requests. The current version is **24**.

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
Add the next version and update both adapters in the same change. Version 13 adds
workspace-scoped repositories, one-repository-per-service links, and independent
service ref tracks, with a backfill from legacy Git code bindings.
Version 14 adds workspace-scoped `specification_code_bindings`, moving
branch/tag/commit selection from the service relationship to each specification.
Version 15 adds the selected Markdown path to each specification code binding.
Bindings with a non-empty path represent repository-managed specifications and
are protected from in-app content edits and source removal.
Version 16 adds immutable, workspace-scoped `repository_snapshots` keyed by
repository, source ref, and resolved commit. Each row keeps the manifest hash,
provider version, fetch time, and optional content-root metadata so downstream
controllers can consume durable evidence after a trigger or process restart.
Version 17 adds durable controller tasks with leases and retry state,
`repository_snapshot_files` content references, workspace-scoped encrypted Secret
metadata, provider Integrations, and branch polling schedules. The platform
ContentStore owns the bytes; the database keeps only references and evidence.
Version 18 adds service-scoped OpenAPI provider settings and environments. Environment
metadata and non-sensitive variables are stored with the service; sensitive values are
encrypted through the existing Secret store and are returned only by the explicit
runtime resolve operation used to start a request. The environment list never returns
secret material, and deleting or replacing a variable removes its orphaned Secret.
Version 19 introduced a service-scoped OpenAPI workbench document for shared collection
trees and saved request definitions. Its API and store accessors have since been removed;
the historical migration and existing rows remain unchanged. Request history remains
device-local.
Version 20 adds the OpenAPI Mock enable flag and a one-way token hash. The raw token is
returned only when a URL is generated or rotated; it is never recoverable from provider
settings after that response.
Version 21 adds immutable OpenAPI publication snapshots, uniquely bound to a
specification revision. Deleting the revision or specification cascades to its
public links. Workspace SQLite backups and managed PostgreSQL dumps omit
publication rows so restoring a backup does not reactivate revoked links;
owners must explicitly publish a new link after restoring. Raw administrator
database copies retain their original contents and are outside this policy.
Version 22 adds publication-site presentation settings (description, light/dark
theme, and whether readers may download the OpenAPI definition) to the immutable
snapshot. Existing links receive the safe defaults during migration.
Version 23 adds the immutable publication `allow_try_it` flag. Existing links
default to enabled so their behavior remains compatible; public requests remain
isolated from workspace credentials and must target an explicit HTTP(S) URL supplied by the reader. Authentication uses only credentials the reader enters.

Hosts open the adapter before exposing the API router. `/ready` remains false
until the latest marker exists. Web startup makes three bounded attempts with a
short backoff, logs the final storage error, and exits without
binding business handlers if the database remains unavailable. The process
supervisor may then restart it. The desktop shell remains visible and retries
opening the adapter on the next request. The desktop host exposes the
read-only `migration_status` command and the retryable `migration_retry`
command so startup UI can show a stable gate state without receiving storage
diagnostic text. Business IPC still returns the shared `not_ready` envelope
until the gate is released. Both hosts therefore follow this
order: install the new binary, acquire the migration lock, migrate, verify
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
databases, and PostgreSQL integration tests against a service container. The
PostgreSQL job sets `SPECX_REQUIRE_POSTGRES=1`, so a missing connection URL
fails instead of silently skipping the suite. The check rejects missing or
divergent versions before a release.

Version 24 adds the environment `request_mode` (inherit/browser/agent), preserving existing selections with inherit.

Version 25 adds `openapi_environments.servers` for named service URLs and proxy modes. Existing environments keep their default URL and start with an empty list.

The historical `openapi_workbenches` migration remains immutable. Saved cases and explicit request stashes have been removed; no active API or store code reads or writes that table. Existing rows are left untouched.
