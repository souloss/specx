//! PostgreSQL persistence adapter for the specX facts.
//!
//! The SQLite adapter remains useful for single-process development and local
//! deployments.  This adapter is the shared-store path: all reads and writes
//! are scoped by `(tenant_id, workspace_id)`, mutations use one database
//! transaction, and schema changes are recorded in `schema_migrations`.
//!
//! The adapter intentionally exposes asynchronous methods because PostgreSQL
//! connections are pooled and must never block an HTTP executor thread.

use crate::persistence::{
    AuditEvent, BackupArtifact, ControllerTask, IdempotencyRecord, IntegrationRecord, LATEST_SCHEMA_VERSION,
    PersistenceError, RepositorySnapshotFile, RepositorySyncSchedule, SecretRecord, SpecificationRevision, WebSession,
    WorkspaceInvitation, WorkspaceMember, WorkspaceMembership,
};
use crate::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Core,
    Repository, RepositorySnapshot, Service, ServiceCodeBinding, ServiceGroup, ServiceRefKind, ServiceRefTrack,
    ServiceRepository, Specification, SpecificationCodeBinding, SpecificationKind, SpecificationRefKind, Tag,
    Workspace, WorkspaceContext, normalize_repository_url,
};
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::verify_server_cert_signed_by_trust_anchor;
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use sha2::{Digest, Sha256};
use std::io::BufReader;
use std::sync::Arc;
use std::{
    ffi::OsString,
    fmt, fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_postgres::{Config, NoTls, Row, Transaction, config::SslMode};

const MIGRATION_LOCK_KEY: (i32, i32) = (0x5350_5801, 0x4d49_4701);

/// A pooled PostgreSQL store.  Cloning the handle is cheap and safe; the pool
/// bounds concurrent database work across API workers and application nodes.
#[derive(Clone)]
pub struct PostgresStore {
    pool: Pool,
    database_url: String,
}

impl fmt::Debug for PostgresStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PostgresStore").field("status", &self.pool.status()).finish()
    }
}

impl PostgresStore {
    /// Connect to PostgreSQL and run all pending migrations before returning.
    pub async fn connect(url: &str, max_size: usize) -> Result<Self, PersistenceError> {
        let mut config: Config =
            postgres_config_url(url).parse::<Config>().map_err(|error| PersistenceError::new(error.to_string()))?;
        config.ssl_mode(postgres_ssl_mode(url)?);
        if postgres_tls_required() && config.get_ssl_mode() == tokio_postgres::config::SslMode::Disable {
            return Err(PersistenceError::new("PostgreSQL TLS is required by SPECX_POSTGRES_REQUIRE_TLS"));
        }
        let manager_config = ManagerConfig { recycling_method: RecyclingMethod::Fast };
        let pool = if config.get_ssl_mode() == tokio_postgres::config::SslMode::Disable {
            let manager = Manager::from_config(config, NoTls, manager_config);
            Pool::builder(manager)
                .max_size(max_size.max(1))
                .runtime(Runtime::Tokio1)
                .build()
                .map_err(|error| PersistenceError::new(format!("PostgreSQL pool: {error}")))?
        } else {
            let tls = postgres_tls_connector(url)?;
            let manager = Manager::from_config(config, tls, manager_config);
            Pool::builder(manager)
                .max_size(max_size.max(1))
                .runtime(Runtime::Tokio1)
                .build()
                .map_err(|error| PersistenceError::new(format!("PostgreSQL TLS pool: {error}")))?
        };
        let store = Self { pool, database_url: url.to_owned() };
        store.migrate().await?;
        Ok(store)
    }

    /// Rebuild the pool from the current URL and TLS files. Deployments can
    /// atomically replace certificate files and call this after a secret
    /// rotation without restarting the API process.
    pub async fn reload_credentials(&mut self, max_pool_size: usize) -> Result<(), PersistenceError> {
        let replacement = Self::connect(&self.database_url, max_pool_size).await?;
        self.pool = replacement.pool;
        Ok(())
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Keep a shared-backup retention operation exclusive across PostgreSQL
    /// clients. The transaction-scoped advisory lock is released even if the
    /// async operation fails or its task is cancelled.
    pub async fn with_backup_retention_lock<T, F, Fut>(
        &self,
        lock_key: (i32, i32),
        operation: F,
    ) -> Result<T, PersistenceError>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, PersistenceError>> + Send + 'static,
    {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        transaction
            .query_one("SELECT pg_advisory_xact_lock($1, $2)", &[&lock_key.0, &lock_key.1])
            .await
            .map_err(pg_error)?;
        match operation().await {
            Ok(value) => {
                transaction.commit().await.map_err(pg_error)?;
                Ok(value)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    /// Create a managed `pg_dump` custom-format snapshot. The command is
    /// intentionally external so deployments can pin the PostgreSQL client
    /// version and use their own credential/TLS configuration.
    pub async fn create_backup(
        &self,
        directory: impl AsRef<std::path::Path> + Send + 'static,
        prefix: &str,
        retention: usize,
    ) -> Result<BackupArtifact, PersistenceError> {
        let directory = directory.as_ref().to_path_buf();
        let prefix = prefix.to_owned();
        let database_url = self.database_url.clone();
        let dump_bin = std::env::var("SPECX_PG_DUMP_BIN").unwrap_or_else(|_| "pg_dump".to_owned());
        let timeout = backup_timeout();
        tokio::task::spawn_blocking(move || {
            create_pg_dump(&database_url, &directory, &prefix, retention, &dump_bin, timeout)
        })
        .await
        .map_err(|error| PersistenceError::new(format!("PostgreSQL backup worker panicked: {error}")))?
    }

    /// Restore a managed custom-format snapshot in one PostgreSQL transaction.
    pub async fn restore_backup(
        &self,
        source: impl AsRef<std::path::Path> + Send + 'static,
    ) -> Result<(), PersistenceError> {
        let source = source.as_ref().to_path_buf();
        let database_url = self.database_url.clone();
        let restore_bin = std::env::var("SPECX_PG_RESTORE_BIN").unwrap_or_else(|_| "pg_restore".to_owned());
        let timeout = backup_timeout();
        tokio::task::spawn_blocking(move || restore_pg_dump(&database_url, &source, &restore_bin, timeout))
            .await
            .map_err(|error| PersistenceError::new(format!("PostgreSQL restore worker panicked: {error}")))?
    }

    pub async fn migrate(&self) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        transaction
            .query_one("SELECT pg_advisory_xact_lock($1, $2)", &[&MIGRATION_LOCK_KEY.0, &MIGRATION_LOCK_KEY.1])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                   version BIGINT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
                 );
                 CREATE TABLE IF NOT EXISTS service_groups (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   parent_id TEXT, name TEXT NOT NULL, position BIGINT NOT NULL DEFAULT 0,
                   UNIQUE (tenant_id, workspace_id, id)
                 );
                 CREATE INDEX IF NOT EXISTS service_groups_scope ON service_groups(tenant_id, workspace_id, parent_id);
                 CREATE TABLE IF NOT EXISTS tags (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   name TEXT NOT NULL, color TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, name)
                 );
                 CREATE TABLE IF NOT EXISTS services (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   group_id TEXT, name TEXT NOT NULL, slug TEXT NOT NULL,
                   description TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL DEFAULT '',
                   archived BOOLEAN NOT NULL DEFAULT FALSE,
                   UNIQUE (tenant_id, workspace_id, slug)
                 );
                 CREATE TABLE IF NOT EXISTS code_sources (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   source_type TEXT NOT NULL, provider TEXT NOT NULL, display_name TEXT NOT NULL,
                   canonical_key TEXT NOT NULL, root_locator TEXT NOT NULL, execution_scope TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived BOOLEAN NOT NULL DEFAULT FALSE,
                   UNIQUE (tenant_id, workspace_id, canonical_key)
                 );
                 CREATE INDEX IF NOT EXISTS code_sources_scope ON code_sources(tenant_id, workspace_id, display_name);
                 CREATE TABLE IF NOT EXISTS service_code_bindings (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                   source_id TEXT NOT NULL REFERENCES code_sources(id) ON DELETE RESTRICT,
                   role TEXT NOT NULL, entity_kind TEXT NOT NULL, path TEXT NOT NULL DEFAULT '',
                   revision_kind TEXT NOT NULL, revision TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, service_id, source_id, entity_kind, path)
                 );
                 CREATE INDEX IF NOT EXISTS service_code_bindings_service ON service_code_bindings(tenant_id, workspace_id, service_id);
                 CREATE TABLE IF NOT EXISTS repositories (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   provider TEXT NOT NULL, display_name TEXT NOT NULL, canonical_url TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived BOOLEAN NOT NULL DEFAULT FALSE,
                   UNIQUE (tenant_id, workspace_id, canonical_url)
                 );
                 CREATE TABLE IF NOT EXISTS repository_snapshots (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
                   source_ref TEXT NOT NULL, resolved_commit TEXT NOT NULL, manifest_hash TEXT NOT NULL,
                   content_root_ref TEXT, fetched_at TEXT NOT NULL, provider_version TEXT NOT NULL, created_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, repository_id, source_ref, resolved_commit)
                 );
                 CREATE INDEX IF NOT EXISTS repository_snapshots_scope ON repository_snapshots(tenant_id, workspace_id, repository_id, fetched_at);
                 CREATE INDEX IF NOT EXISTS repositories_scope ON repositories(tenant_id, workspace_id, display_name);
                 CREATE TABLE IF NOT EXISTS service_repositories (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                   repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE RESTRICT,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, service_id)
                 );
                 CREATE TABLE IF NOT EXISTS service_ref_tracks (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   service_repository_id TEXT NOT NULL REFERENCES service_repositories(id) ON DELETE CASCADE,
                   ref_kind TEXT NOT NULL, ref_name TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, service_repository_id, ref_kind, ref_name)
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS service_ref_tracks_default ON service_ref_tracks(tenant_id, workspace_id, service_repository_id)
                   WHERE ref_kind = 'default_branch';
                 CREATE INDEX IF NOT EXISTS services_scope ON services(tenant_id, workspace_id, name);
                 CREATE TABLE IF NOT EXISTS service_tags (
                   tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   service_id TEXT NOT NULL, tag_id TEXT NOT NULL,
                   PRIMARY KEY (tenant_id, workspace_id, service_id, tag_id)
                 );
                 CREATE TABLE IF NOT EXISTS specifications (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   service_id TEXT NOT NULL, title TEXT NOT NULL, kind TEXT NOT NULL,
                   content TEXT NOT NULL, version BIGINT NOT NULL DEFAULT 1,
                   content_hash TEXT NOT NULL, updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS specifications_scope ON specifications(tenant_id, workspace_id, service_id, updated_at);
                 CREATE TABLE IF NOT EXISTS specification_revisions (
                   id BIGSERIAL PRIMARY KEY, specification_id TEXT NOT NULL,
                   tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL, version BIGINT NOT NULL,
                   content TEXT NOT NULL, content_hash TEXT NOT NULL, created_by TEXT NOT NULL,
                   created_at TEXT NOT NULL, UNIQUE (specification_id, version)
                 );
                 CREATE TABLE IF NOT EXISTS specification_code_bindings (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   specification_id TEXT NOT NULL REFERENCES specifications(id) ON DELETE CASCADE,
                   service_repository_id TEXT NOT NULL REFERENCES service_repositories(id) ON DELETE CASCADE,
                   path TEXT NOT NULL DEFAULT '', ref_kind TEXT NOT NULL, ref_name TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, specification_id)
                 );
                 CREATE INDEX IF NOT EXISTS specification_code_bindings_scope ON specification_code_bindings(tenant_id, workspace_id, specification_id);
                 CREATE TABLE IF NOT EXISTS workspace_members (
                   tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL, actor_id TEXT NOT NULL,
                   role TEXT NOT NULL, revoked_at TEXT, created_at TEXT NOT NULL,
                   PRIMARY KEY (tenant_id, workspace_id, actor_id)
                 );
                 CREATE TABLE IF NOT EXISTS workspaces (
                   tenant_id TEXT NOT NULL, id TEXT NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL, created_at TEXT NOT NULL,
                   PRIMARY KEY (tenant_id, id), UNIQUE (tenant_id, slug)
                 );
                 CREATE TABLE IF NOT EXISTS audit_events (
                   id BIGSERIAL PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   actor_id TEXT NOT NULL, request_id TEXT NOT NULL, action TEXT NOT NULL,
                   resource_type TEXT NOT NULL, resource_id TEXT NOT NULL, created_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS audit_events_scope ON audit_events(tenant_id, workspace_id, id);
                 CREATE TABLE IF NOT EXISTS api_rate_limits (
                   tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL, actor_id TEXT NOT NULL,
                   window_start BIGINT NOT NULL, request_count BIGINT NOT NULL,
                   PRIMARY KEY (tenant_id, workspace_id, actor_id)
                 );
                 CREATE TABLE IF NOT EXISTS api_idempotency (
                   idempotency_key TEXT PRIMARY KEY,
                   tenant_id TEXT NOT NULL,
                   workspace_id TEXT NOT NULL,
                   fingerprint TEXT NOT NULL,
                   resource_type TEXT NOT NULL,
                   resource_id TEXT NOT NULL,
                   created_at TIMESTAMPTZ NOT NULL DEFAULT now()
                 );
                 CREATE TABLE IF NOT EXISTS workspace_invitations (
                   id TEXT PRIMARY KEY,
                   tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   email TEXT NOT NULL, role TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
                   expires_at TEXT NOT NULL, accepted_at TEXT, revoked_at TEXT, created_by TEXT NOT NULL, created_at TEXT NOT NULL
                 );
                 INSERT INTO schema_migrations(version) VALUES (1) ON CONFLICT DO NOTHING;",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute("ALTER TABLE service_groups ADD COLUMN IF NOT EXISTS position BIGINT NOT NULL DEFAULT 0")
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "INSERT INTO workspaces(tenant_id,id,name,slug,created_at)
                 SELECT tenant_id,workspace_id,
                        CASE WHEN workspace_id='default' THEN '默认工作区' ELSE workspace_id END,
                        workspace_id,MIN(created_at)
                 FROM (
                   SELECT tenant_id,workspace_id,created_at FROM workspace_members
                   UNION ALL SELECT tenant_id,workspace_id,now()::text FROM service_groups
                   UNION ALL SELECT tenant_id,workspace_id,now()::text FROM tags
                   UNION ALL SELECT tenant_id,workspace_id,now()::text FROM services
                   UNION ALL SELECT tenant_id,workspace_id,now()::text FROM specifications
                 ) existing_workspaces
                 GROUP BY tenant_id,workspace_id
                 ON CONFLICT (tenant_id,id) DO NOTHING",
                &[],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (2) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (3) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (4) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute("ALTER TABLE workspace_invitations ADD COLUMN IF NOT EXISTS revoked_at TEXT")
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (5) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_workspace_invitations_expires_at ON workspace_invitations(expires_at)",
                &[],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (6) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS web_sessions (
                   token_hash TEXT PRIMARY KEY,
                   actor_id TEXT NOT NULL,
                   expires_at TEXT NOT NULL,
                   revoked_at TEXT,
                   created_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_web_sessions_expires_at ON web_sessions(expires_at);",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (7) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        // Versions 8-10 are retained as explicit no-op markers here because
        // PostgreSQL creates these columns/indexes in the initial schema.
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (8) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (9) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE INDEX IF NOT EXISTS idx_workspace_invitations_expires_at ON workspace_invitations(expires_at);",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (10) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (11) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (12) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TEMP TABLE specx_v13_legacy_repository_map(
                   source_id TEXT PRIMARY KEY, repository_id TEXT NOT NULL
                 ) ON COMMIT DROP",
            )
            .await
            .map_err(pg_error)?;
        let legacy_git_sources = transaction
            .query(
                "SELECT id,tenant_id,workspace_id,root_locator,created_at,updated_at,archived
                 FROM code_sources WHERE source_type='git' ORDER BY created_at,id",
                &[],
            )
            .await
            .map_err(pg_error)?;
        for row in legacy_git_sources {
            let source_id: String = row.get(0);
            let tenant_id: String = row.get(1);
            let workspace_id: String = row.get(2);
            let root_locator: String = row.get(3);
            let created_at: String = row.get(4);
            let updated_at: String = row.get(5);
            let archived: bool = row.get(6);
            let Ok((canonical_url, provider, display_name)) = normalize_repository_url(&root_locator) else {
                continue;
            };
            let existing = transaction
                .query_opt(
                    "SELECT id FROM repositories WHERE tenant_id=$1 AND workspace_id=$2 AND canonical_url=$3",
                    &[&tenant_id, &workspace_id, &canonical_url],
                )
                .await
                .map_err(pg_error)?;
            let repository_id: String = if let Some(existing) = existing {
                existing.get(0)
            } else {
                transaction
                    .execute(
                        "INSERT INTO repositories(id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived)
                         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING",
                        &[&source_id, &tenant_id, &workspace_id, &provider, &display_name, &canonical_url, &created_at, &updated_at, &archived],
                    )
                    .await
                    .map_err(pg_error)?;
                transaction
                    .query_one(
                        "SELECT id FROM repositories WHERE tenant_id=$1 AND workspace_id=$2 AND canonical_url=$3",
                        &[&tenant_id, &workspace_id, &canonical_url],
                    )
                    .await
                    .map_err(pg_error)?
                    .get(0)
            };
            transaction
                .execute(
                    "INSERT INTO specx_v13_legacy_repository_map(source_id,repository_id) VALUES($1,$2)",
                    &[&source_id, &repository_id],
                )
                .await
                .map_err(pg_error)?;
        }
        transaction
            .execute(
                "INSERT INTO service_repositories(id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at)
                 SELECT binding.id,binding.tenant_id,binding.workspace_id,binding.service_id,repository.id,binding.created_at,binding.updated_at
                 FROM service_code_bindings binding
                 JOIN specx_v13_legacy_repository_map source_repository ON source_repository.source_id=binding.source_id
                 JOIN repositories repository ON repository.id=source_repository.repository_id
                 WHERE binding.id=(
                   SELECT candidate.id FROM service_code_bindings candidate
                   JOIN specx_v13_legacy_repository_map candidate_repository ON candidate_repository.source_id=candidate.source_id
                   WHERE candidate.tenant_id=binding.tenant_id AND candidate.workspace_id=binding.workspace_id
                     AND candidate.service_id=binding.service_id
                   ORDER BY CASE candidate.role WHEN 'primary' THEN 0 ELSE 1 END,candidate.created_at,candidate.id LIMIT 1
                 ) ON CONFLICT(tenant_id,workspace_id,service_id) DO NOTHING",
                &[],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "INSERT INTO service_ref_tracks(id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at)
                 SELECT binding.id,binding.tenant_id,binding.workspace_id,relation.id,
                   CASE binding.revision_kind WHEN 'default_branch' THEN 'default_branch' ELSE binding.revision_kind END,
                   CASE binding.revision_kind WHEN 'default_branch' THEN NULL ELSE binding.revision END,
                   binding.created_at,binding.updated_at
                 FROM service_code_bindings binding
                 JOIN specx_v13_legacy_repository_map source_repository ON source_repository.source_id=binding.source_id
                 JOIN service_repositories relation ON relation.service_id=binding.service_id
                   AND relation.tenant_id=binding.tenant_id AND relation.workspace_id=binding.workspace_id
                   AND relation.repository_id=source_repository.repository_id
                 WHERE binding.revision_kind IN ('default_branch','branch','tag','commit')
                 ORDER BY binding.created_at,binding.id
                 ON CONFLICT DO NOTHING",
                &[],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (13) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS specification_code_bindings (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   specification_id TEXT NOT NULL REFERENCES specifications(id) ON DELETE CASCADE,
                   service_repository_id TEXT NOT NULL REFERENCES service_repositories(id) ON DELETE CASCADE,
                   path TEXT NOT NULL DEFAULT '', ref_kind TEXT NOT NULL, ref_name TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, specification_id)
                 );
                 CREATE INDEX IF NOT EXISTS specification_code_bindings_scope ON specification_code_bindings(tenant_id, workspace_id, specification_id);",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (14) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "ALTER TABLE specification_code_bindings ADD COLUMN IF NOT EXISTS path TEXT NOT NULL DEFAULT '';",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (15) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS repository_snapshots (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
                   source_ref TEXT NOT NULL, resolved_commit TEXT NOT NULL, manifest_hash TEXT NOT NULL,
                   content_root_ref TEXT, fetched_at TEXT NOT NULL, provider_version TEXT NOT NULL, created_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, repository_id, source_ref, resolved_commit)
                 );
                 CREATE INDEX IF NOT EXISTS repository_snapshots_scope ON repository_snapshots(tenant_id, workspace_id, repository_id, fetched_at);",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (16) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS controller_tasks (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   controller_id TEXT NOT NULL, resource_kind TEXT NOT NULL, resource_id TEXT NOT NULL,
                   generation BIGINT NOT NULL, deduplication_key TEXT NOT NULL,
                   status TEXT NOT NULL, attempts BIGINT NOT NULL DEFAULT 0, next_run_at TEXT NOT NULL,
                   lease_until TEXT, last_error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, controller_id, deduplication_key)
                 );
                 CREATE INDEX IF NOT EXISTS controller_tasks_due ON controller_tasks(status, next_run_at, lease_until);
                 CREATE TABLE IF NOT EXISTS repository_snapshot_files (
                   snapshot_id TEXT NOT NULL REFERENCES repository_snapshots(id) ON DELETE CASCADE,
                   path TEXT NOT NULL, content_ref TEXT NOT NULL, content_hash TEXT NOT NULL,
                   byte_length BIGINT NOT NULL, media_type TEXT NOT NULL,
                   PRIMARY KEY (snapshot_id, path)
                 );
                 CREATE TABLE IF NOT EXISTS secrets (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   name TEXT NOT NULL, encrypted_value TEXT NOT NULL, key_version BIGINT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, name)
                 );
                 CREATE TABLE IF NOT EXISTS integrations (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   provider TEXT NOT NULL, display_name TEXT NOT NULL, secret_id TEXT REFERENCES secrets(id) ON DELETE SET NULL,
                   permissions TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, provider, display_name)
                 );
                 CREATE TABLE IF NOT EXISTS repository_sync_schedules (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
                   ref_kind TEXT NOT NULL, ref_name TEXT NOT NULL, interval_seconds BIGINT NOT NULL,
                   enabled BOOLEAN NOT NULL DEFAULT TRUE, next_run_at TEXT NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                   UNIQUE (tenant_id, workspace_id, repository_id, ref_kind, ref_name)
                 );
                 CREATE INDEX IF NOT EXISTS repository_sync_schedules_due ON repository_sync_schedules(enabled, next_run_at);",
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("INSERT INTO schema_migrations(version) VALUES (17) ON CONFLICT DO NOTHING", &[])
            .await
            .map_err(pg_error)?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(())
    }

    pub async fn ready(&self) -> Result<bool, PersistenceError> {
        let client = self.client().await?;
        let row = client
            .query_opt("SELECT version FROM schema_migrations WHERE version = $1", &[&LATEST_SCHEMA_VERSION])
            .await
            .map_err(pg_error)?;
        Ok(row.is_some())
    }

    pub async fn create_web_session(&self, session: &WebSession) -> Result<(), PersistenceError> {
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO web_sessions(token_hash,actor_id,expires_at,revoked_at,created_at)
                 VALUES($1,$2,$3,NULL,$4)
                 ON CONFLICT(token_hash) DO UPDATE SET actor_id=excluded.actor_id, expires_at=excluded.expires_at,
                   revoked_at=NULL, created_at=excluded.created_at",
                &[&session.token_hash, &session.actor_id, &session.expires_at, &session.created_at],
            )
            .await
            .map_err(pg_error)?;
        Ok(())
    }

    pub async fn web_session_is_active(&self, token_hash: &str, now: &str) -> Result<bool, PersistenceError> {
        let client = self.client().await?;
        let row = client
            .query_one(
                "SELECT EXISTS(
                   SELECT 1 FROM web_sessions
                   WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at > $2
                 )",
                &[&token_hash, &now],
            )
            .await
            .map_err(pg_error)?;
        Ok(row.get(0))
    }

    pub async fn web_session_actor_id(&self, token_hash: &str, now: &str) -> Result<Option<String>, PersistenceError> {
        let client = self.client().await?;
        client
            .query_opt(
                "SELECT actor_id FROM web_sessions
                 WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at > $2",
                &[&token_hash, &now],
            )
            .await
            .map(|row| row.map(|row| row.get(0)))
            .map_err(pg_error)
    }

    pub async fn revoke_web_session(&self, token_hash: &str, revoked_at: &str) -> Result<bool, PersistenceError> {
        let client = self.client().await?;
        Ok(client
            .execute(
                "UPDATE web_sessions SET revoked_at=$1 WHERE token_hash=$2 AND revoked_at IS NULL",
                &[&revoked_at, &token_hash],
            )
            .await
            .map_err(pg_error)?
            == 1)
    }

    pub async fn revoke_other_web_sessions(&self, token_hash: &str, revoked_at: &str) -> Result<u64, PersistenceError> {
        let client = self.client().await?;
        client
            .execute(
                "UPDATE web_sessions SET revoked_at=$1 WHERE token_hash<>$2 AND revoked_at IS NULL",
                &[&revoked_at, &token_hash],
            )
            .await
            .map_err(pg_error)
    }

    pub async fn persist_core(&self, core: &Core, context: &WorkspaceContext) -> Result<(), PersistenceError> {
        self.persist_core_with_audit(core, context, None).await
    }

    pub async fn persist_core_with_audit(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
    ) -> Result<(), PersistenceError> {
        self.persist_core_with_audit_and_idempotency(core, context, audit, None).await
    }

    pub async fn persist_core_with_audit_and_idempotency(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
        idempotency: Option<(&str, &str, &str, &str, &str, &str)>,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        for group in core.groups().filter(|group| in_scope(context, &group.tenant_id, &group.workspace_id)) {
            let changed = transaction
                .execute(
                    "INSERT INTO service_groups(id, tenant_id, workspace_id, parent_id, name, position)
                     VALUES ($1,$2,$3,$4,$5,$6)
                     ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,name=excluded.name,position=excluded.position
                     WHERE service_groups.tenant_id=excluded.tenant_id AND service_groups.workspace_id=excluded.workspace_id",
                    &[&group.id, &context.tenant_id, &context.workspace_id, &group.parent_id, &group.name, &group.position],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "group")?;
        }
        for tag in core.tags().filter(|tag| in_scope(context, &tag.tenant_id, &tag.workspace_id)) {
            let changed = transaction
                .execute(
                    "INSERT INTO tags(id,tenant_id,workspace_id,name,color) VALUES($1,$2,$3,$4,$5)
                     ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color
                     WHERE tags.tenant_id=excluded.tenant_id AND tags.workspace_id=excluded.workspace_id",
                    &[&tag.id, &context.tenant_id, &context.workspace_id, &tag.name, &tag.color],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "tag")?;
        }
        for service in core.services().filter(|service| in_scope(context, &service.tenant_id, &service.workspace_id)) {
            let changed = transaction
                .execute(
                    "INSERT INTO services(id,tenant_id,workspace_id,group_id,name,slug,description,updated_at,archived)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
                     ON CONFLICT(id) DO UPDATE SET group_id=excluded.group_id,name=excluded.name,slug=excluded.slug,
                       description=excluded.description,updated_at=excluded.updated_at,archived=excluded.archived
                     WHERE services.tenant_id=excluded.tenant_id AND services.workspace_id=excluded.workspace_id",
                    &[
                        &service.id,
                        &context.tenant_id,
                        &context.workspace_id,
                        &service.group_id,
                        &service.name,
                        &service.slug,
                        &service.description,
                        &service.updated_at,
                        &service.archived,
                    ],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "service")?;
            transaction
                .execute(
                    "DELETE FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3",
                    &[&context.tenant_id, &context.workspace_id, &service.id],
                )
                .await
                .map_err(pg_error)?;
            for tag_id in &service.tag_ids {
                transaction
                    .execute(
                        "INSERT INTO service_tags(tenant_id,workspace_id,service_id,tag_id) VALUES($1,$2,$3,$4)",
                        &[&context.tenant_id, &context.workspace_id, &service.id, tag_id],
                    )
                    .await
                    .map_err(pg_error)?;
            }
        }
        for source in core.code_sources().filter(|source| in_scope(context, &source.tenant_id, &source.workspace_id)) {
            let changed = transaction.execute(
                "INSERT INTO code_sources(id,tenant_id,workspace_id,source_type,provider,display_name,canonical_key,root_locator,execution_scope,created_at,updated_at,archived)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
                 ON CONFLICT(id) DO UPDATE SET source_type=excluded.source_type,provider=excluded.provider,display_name=excluded.display_name,
                   canonical_key=excluded.canonical_key,root_locator=excluded.root_locator,execution_scope=excluded.execution_scope,
                   created_at=excluded.created_at,updated_at=excluded.updated_at,archived=excluded.archived
                 WHERE code_sources.tenant_id=excluded.tenant_id AND code_sources.workspace_id=excluded.workspace_id",
                &[&source.id, &source.tenant_id, &source.workspace_id, &source_type_name(source.source_type), &source.provider, &source.display_name, &source.canonical_key, &source.root_locator, &execution_scope_name(source.execution_scope), &source.created_at, &source.updated_at, &source.archived],
            ).await.map_err(pg_error)?;
            ensure_scope(changed, "code source")?;
        }
        for binding in
            core.code_bindings().filter(|binding| in_scope(context, &binding.tenant_id, &binding.workspace_id))
        {
            let changed = transaction.execute(
                "INSERT INTO service_code_bindings(id,tenant_id,workspace_id,service_id,source_id,role,entity_kind,path,revision_kind,revision,created_at,updated_at)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
                 ON CONFLICT(id) DO UPDATE SET service_id=excluded.service_id,source_id=excluded.source_id,role=excluded.role,
                   entity_kind=excluded.entity_kind,path=excluded.path,revision_kind=excluded.revision_kind,revision=excluded.revision,
                   created_at=excluded.created_at,updated_at=excluded.updated_at
                 WHERE service_code_bindings.tenant_id=excluded.tenant_id AND service_code_bindings.workspace_id=excluded.workspace_id",
                &[&binding.id, &binding.tenant_id, &binding.workspace_id, &binding.service_id, &binding.source_id, &binding_role_name(binding.role), &entity_kind_name(binding.entity_kind), &binding.path, &revision_kind_name(binding.revision_kind), &binding.revision, &binding.created_at, &binding.updated_at],
            ).await.map_err(pg_error)?;
            ensure_scope(changed, "code binding")?;
        }
        for repository in
            core.repositories().filter(|repository| in_scope(context, &repository.tenant_id, &repository.workspace_id))
        {
            let changed = transaction.execute(
                "INSERT INTO repositories(id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT(id) DO UPDATE SET provider=excluded.provider,display_name=excluded.display_name,
                   canonical_url=excluded.canonical_url,updated_at=excluded.updated_at,archived=excluded.archived
                 WHERE repositories.tenant_id=excluded.tenant_id AND repositories.workspace_id=excluded.workspace_id",
                &[&repository.id, &repository.tenant_id, &repository.workspace_id, &repository.provider, &repository.display_name, &repository.canonical_url, &repository.created_at, &repository.updated_at, &repository.archived],
            ).await.map_err(pg_error)?;
            ensure_scope(changed, "repository")?;
        }
        for snapshot in core
            .repository_snapshots()
            .filter(|snapshot| in_scope(context, &snapshot.tenant_id, &snapshot.workspace_id))
        {
            let changed = transaction
                .execute(
                    "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                     ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
                       content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version
                     WHERE repository_snapshots.tenant_id=excluded.tenant_id AND repository_snapshots.workspace_id=excluded.workspace_id",
                    &[&snapshot.id, &snapshot.tenant_id, &snapshot.workspace_id, &snapshot.repository_id, &snapshot.source_ref, &snapshot.resolved_commit, &snapshot.manifest_hash, &snapshot.content_root_ref, &snapshot.fetched_at, &snapshot.provider_version, &snapshot.created_at],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "repository snapshot")?;
        }
        for relation in core
            .service_repositories()
            .filter(|relation| in_scope(context, &relation.tenant_id, &relation.workspace_id))
        {
            let changed = transaction.execute(
                "INSERT INTO service_repositories(id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at)
                 VALUES($1,$2,$3,$4,$5,$6,$7)
                 ON CONFLICT(id) DO UPDATE SET service_id=excluded.service_id,repository_id=excluded.repository_id,updated_at=excluded.updated_at
                 WHERE service_repositories.tenant_id=excluded.tenant_id AND service_repositories.workspace_id=excluded.workspace_id",
                &[&relation.id, &relation.tenant_id, &relation.workspace_id, &relation.service_id, &relation.repository_id, &relation.created_at, &relation.updated_at],
            ).await.map_err(pg_error)?;
            ensure_scope(changed, "service repository")?;
        }
        for track in core.ref_tracks().filter(|track| in_scope(context, &track.tenant_id, &track.workspace_id)) {
            let changed = transaction.execute(
                "INSERT INTO service_ref_tracks(id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8)
                 ON CONFLICT(id) DO UPDATE SET service_repository_id=excluded.service_repository_id,ref_kind=excluded.ref_kind,
                   ref_name=excluded.ref_name,updated_at=excluded.updated_at
                 WHERE service_ref_tracks.tenant_id=excluded.tenant_id AND service_ref_tracks.workspace_id=excluded.workspace_id",
                &[&track.id, &track.tenant_id, &track.workspace_id, &track.service_repository_id, &ref_kind_name(track.ref_kind), &track.ref_name, &track.created_at, &track.updated_at],
            ).await.map_err(pg_error)?;
            ensure_scope(changed, "service ref track")?;
        }
        for binding in core
            .specification_code_bindings()
            .filter(|binding| in_scope(context, &binding.tenant_id, &binding.workspace_id))
        {
            let changed = transaction
                .execute(
                    "INSERT INTO specification_code_bindings(id,tenant_id,workspace_id,specification_id,service_repository_id,path,ref_kind,ref_name,created_at,updated_at)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                     ON CONFLICT(id) DO UPDATE SET specification_id=excluded.specification_id,service_repository_id=excluded.service_repository_id,
                       path=excluded.path,ref_kind=excluded.ref_kind,ref_name=excluded.ref_name,updated_at=excluded.updated_at
                     WHERE specification_code_bindings.tenant_id=excluded.tenant_id AND specification_code_bindings.workspace_id=excluded.workspace_id",
                    &[&binding.id, &binding.tenant_id, &binding.workspace_id, &binding.specification_id, &binding.service_repository_id, &binding.path, &specification_ref_kind_name(binding.ref_kind), &binding.ref_name, &binding.created_at, &binding.updated_at],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "specification code binding")?;
        }
        for specification in core
            .specifications()
            .filter(|specification| in_scope(context, &specification.tenant_id, &specification.workspace_id))
        {
            let hash = content_hash(&specification.content);
            let current = transaction
                .query_opt(
                    "SELECT version, content_hash FROM specifications WHERE id=$1 AND tenant_id=$2 AND workspace_id=$3",
                    &[&specification.id, &context.tenant_id, &context.workspace_id],
                )
                .await
                .map_err(pg_error)?;
            let (version, content_changed) = match current {
                Some(row) => {
                    (row.get::<_, i64>(0) + i64::from(row.get::<_, String>(1) != hash), row.get::<_, String>(1) != hash)
                }
                None => (specification.version.max(1), true),
            };
            let updated_at = if specification.updated_at.is_empty() {
                chrono::Utc::now().to_rfc3339()
            } else {
                specification.updated_at.clone()
            };
            let changed = transaction
                .execute(
                    "INSERT INTO specifications(id,tenant_id,workspace_id,service_id,title,kind,content,version,content_hash,updated_at)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                     ON CONFLICT(id) DO UPDATE SET service_id=excluded.service_id,title=excluded.title,kind=excluded.kind,
                       content=excluded.content,version=excluded.version,content_hash=excluded.content_hash,updated_at=excluded.updated_at
                     WHERE specifications.tenant_id=excluded.tenant_id AND specifications.workspace_id=excluded.workspace_id",
                    &[&specification.id, &context.tenant_id, &context.workspace_id, &specification.service_id, &specification.title, &kind_name(specification.kind), &specification.content, &version, &hash, &updated_at],
                )
                .await
                .map_err(pg_error)?;
            ensure_scope(changed, "specification")?;
            if content_changed {
                transaction
                    .execute(
                        "INSERT INTO specification_revisions(specification_id,tenant_id,workspace_id,version,content,content_hash,created_by,created_at)
                         VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                        &[&specification.id, &context.tenant_id, &context.workspace_id, &version, &specification.content, &hash, &context.actor_id, &updated_at],
                    )
                    .await
                    .map_err(pg_error)?;
            }
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        if let Some((key, tenant_id, workspace_id, fingerprint, resource_type, resource_id)) = idempotency {
            transaction
                .execute(
                    "INSERT INTO api_idempotency(idempotency_key,tenant_id,workspace_id,fingerprint,resource_type,resource_id)
                     VALUES($1,$2,$3,$4,$5,$6)",
                    &[&key, &tenant_id, &workspace_id, &fingerprint, &resource_type, &resource_id],
                )
                .await
                .map_err(pg_error)?;
        }
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn load_idempotency(&self, key: &str) -> Result<Option<IdempotencyRecord>, PersistenceError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT fingerprint, resource_type, resource_id FROM api_idempotency WHERE idempotency_key=$1",
                &[&key],
            )
            .await
            .map_err(pg_error)?;
        Ok(row.map(|row| IdempotencyRecord {
            fingerprint: row.get(0),
            resource_type: row.get(1),
            resource_id: row.get(2),
        }))
    }

    pub async fn load_core(&self, context: &WorkspaceContext) -> Result<Core, PersistenceError> {
        let client = self.client().await?;
        let groups = client.query("SELECT id,tenant_id,workspace_id,parent_id,name,position FROM service_groups WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        let tags = client
            .query(
                "SELECT id,tenant_id,workspace_id,name,color FROM tags WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &context.workspace_id],
            )
            .await
            .map_err(pg_error)?;
        let service_rows = client.query("SELECT id,tenant_id,workspace_id,group_id,name,slug,description,updated_at,archived FROM services WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        let spec_rows = client.query("SELECT id,tenant_id,workspace_id,service_id,title,kind,content,version,content_hash,updated_at FROM specifications WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        let mut core = Core::default();
        for row in groups {
            core.insert_group(group_from_row(&row));
        }
        for row in tags {
            core.insert_tag(tag_from_row(&row));
        }
        for row in service_rows {
            let service_id: String = row.get(0);
            let tag_rows = client.query("SELECT tag_id FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3 ORDER BY tag_id", &[&context.tenant_id, &context.workspace_id, &service_id]).await.map_err(pg_error)?;
            let mut service = service_from_row(&row);
            service.tag_ids = tag_rows.into_iter().map(|tag| tag.get(0)).collect();
            core.insert_service(service);
        }
        for row in spec_rows {
            core.insert_specification(specification_from_row(&row)?);
        }
        let source_rows = client.query("SELECT id,tenant_id,workspace_id,source_type,provider,display_name,canonical_key,root_locator,execution_scope,created_at,updated_at,archived FROM code_sources WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in source_rows {
            core.insert_code_source(code_source_from_row(&row)?);
        }
        let binding_rows = client.query("SELECT id,tenant_id,workspace_id,service_id,source_id,role,entity_kind,path,revision_kind,revision,created_at,updated_at FROM service_code_bindings WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in binding_rows {
            core.insert_code_binding(code_binding_from_row(&row)?);
        }
        let repository_rows = client.query("SELECT id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived FROM repositories WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in repository_rows {
            core.insert_repository(repository_from_row(&row));
        }
        let repository_snapshot_rows = client.query("SELECT id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at FROM repository_snapshots WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in repository_snapshot_rows {
            core.insert_repository_snapshot(repository_snapshot_from_row(&row));
        }
        let service_repository_rows = client.query("SELECT id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at FROM service_repositories WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in service_repository_rows {
            core.insert_service_repository(service_repository_from_row(&row));
        }
        let ref_track_rows = client.query("SELECT id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at FROM service_ref_tracks WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in ref_track_rows {
            core.insert_ref_track(ref_track_from_row(&row)?);
        }
        let specification_binding_rows = client.query("SELECT id,tenant_id,workspace_id,specification_id,service_repository_id,path,ref_kind,ref_name,created_at,updated_at FROM specification_code_bindings WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &context.workspace_id]).await.map_err(pg_error)?;
        for row in specification_binding_rows {
            core.insert_specification_code_binding(specification_code_binding_from_row(&row)?);
        }
        Ok(core)
    }

    pub async fn audit_event_count(&self, context: &WorkspaceContext) -> Result<i64, PersistenceError> {
        let client = self.client().await?;
        let row = client
            .query_one(
                "SELECT count(*) FROM audit_events WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &context.workspace_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(row.get(0))
    }

    pub async fn specification_revision_count(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Result<i64, PersistenceError> {
        let client = self.client().await?;
        let row = client
            .query_one(
                "SELECT count(*) FROM specification_revisions WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id=$3",
                &[&context.tenant_id, &context.workspace_id, &specification_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(row.get(0))
    }

    pub async fn specification_revisions(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SpecificationRevision>, PersistenceError> {
        let client = self.client().await?;
        client
            .query(
                "SELECT specification_id, version, content, content_hash, created_by, created_at
                 FROM specification_revisions
                 WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id=$3
                 ORDER BY version DESC LIMIT $4 OFFSET $5",
                &[&context.tenant_id, &context.workspace_id, &specification_id, &(limit as i64), &(offset as i64)],
            )
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| SpecificationRevision {
                        specification_id: row.get(0),
                        version: row.get(1),
                        content: row.get(2),
                        content_hash: row.get(3),
                        created_by: row.get(4),
                        created_at: row.get(5),
                    })
                    .collect()
            })
            .map_err(pg_error)
    }

    pub async fn specification_revision(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        version: i64,
    ) -> Result<Option<SpecificationRevision>, PersistenceError> {
        let client = self.client().await?;
        client
            .query_opt(
                "SELECT specification_id, version, content, content_hash, created_by, created_at
                 FROM specification_revisions
                 WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id=$3 AND version=$4",
                &[&context.tenant_id, &context.workspace_id, &specification_id, &version],
            )
            .await
            .map(|row| {
                row.map(|row| SpecificationRevision {
                    specification_id: row.get(0),
                    version: row.get(1),
                    content: row.get(2),
                    content_hash: row.get(3),
                    created_by: row.get(4),
                    created_at: row.get(5),
                })
            })
            .map_err(pg_error)
    }

    pub async fn update_specification_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &Specification,
        updated: &Specification,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        self.update_specification_if_current_with_audit_and_snapshot(context, expected, updated, audit, None).await
    }

    pub async fn update_specification_if_current_with_audit_and_snapshot(
        &self,
        context: &WorkspaceContext,
        expected: &Specification,
        updated: &Specification,
        audit: Option<&AuditEvent>,
        snapshot: Option<&RepositorySnapshot>,
    ) -> Result<bool, PersistenceError> {
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError::new("specification update scope does not match"));
        }
        if let Some(snapshot) = snapshot
            && (snapshot.tenant_id != context.tenant_id || snapshot.workspace_id != context.workspace_id)
        {
            return Err(PersistenceError::new("repository snapshot scope does not match"));
        }

        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let hash = content_hash(&updated.content);
        let content_changed = expected.content != updated.content;
        let version = expected.version + i64::from(content_changed);
        let changed = transaction
            .execute(
                "UPDATE specifications SET title=$1,content=$2,version=$3,content_hash=$4,updated_at=$5
                 WHERE id=$6 AND tenant_id=$7 AND workspace_id=$8 AND service_id=$9 AND title=$10
                   AND kind=$11 AND content=$12 AND version=$13 AND content_hash=$14 AND updated_at=$15",
                &[
                    &updated.title,
                    &updated.content,
                    &version,
                    &hash,
                    &updated.updated_at,
                    &expected.id,
                    &context.tenant_id,
                    &context.workspace_id,
                    &expected.service_id,
                    &expected.title,
                    &kind_name(expected.kind),
                    &expected.content,
                    &expected.version,
                    &expected.content_hash,
                    &expected.updated_at,
                ],
            )
            .await
            .map_err(pg_error)?;
        if changed != 1 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        if content_changed {
            transaction
                .execute(
                    "INSERT INTO specification_revisions(specification_id,tenant_id,workspace_id,version,content,content_hash,created_by,created_at)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                    &[
                        &updated.id,
                        &context.tenant_id,
                        &context.workspace_id,
                        &version,
                        &updated.content,
                        &hash,
                        &context.actor_id,
                        &updated.updated_at,
                    ],
                )
                .await
                .map_err(pg_error)?;
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        if let Some(snapshot) = snapshot {
            transaction
                .execute(
                    "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
                     VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                     ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
                       content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version",
                    &[&snapshot.id, &snapshot.tenant_id, &snapshot.workspace_id, &snapshot.repository_id, &snapshot.source_ref, &snapshot.resolved_commit, &snapshot.manifest_hash, &snapshot.content_root_ref, &snapshot.fetched_at, &snapshot.provider_version, &snapshot.created_at],
                )
                .await
                .map_err(pg_error)?;
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn upsert_repository_snapshot(
        &self,
        context: &WorkspaceContext,
        snapshot: &RepositorySnapshot,
    ) -> Result<(), PersistenceError> {
        if snapshot.tenant_id != context.tenant_id || snapshot.workspace_id != context.workspace_id {
            return Err(PersistenceError::new("repository snapshot scope does not match"));
        }
        let client = self.client().await?;
        client
            .execute(
                "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                 ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
                   content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version",
                &[&snapshot.id, &snapshot.tenant_id, &snapshot.workspace_id, &snapshot.repository_id, &snapshot.source_ref, &snapshot.resolved_commit, &snapshot.manifest_hash, &snapshot.content_root_ref, &snapshot.fetched_at, &snapshot.provider_version, &snapshot.created_at],
            )
            .await
            .map_err(pg_error)?;
        Ok(())
    }

    pub async fn upsert_repository_snapshot_file(&self, file: &RepositorySnapshotFile) -> Result<(), PersistenceError> {
        let client = self.client().await?;
        client.execute(
            "INSERT INTO repository_snapshot_files(snapshot_id,path,content_ref,content_hash,byte_length,media_type)
             VALUES($1,$2,$3,$4,$5,$6)
             ON CONFLICT(snapshot_id,path) DO UPDATE SET content_ref=excluded.content_ref,content_hash=excluded.content_hash,byte_length=excluded.byte_length,media_type=excluded.media_type",
            &[&file.snapshot_id, &file.path, &file.content_ref, &file.content_hash, &file.byte_length, &file.media_type],
        ).await.map_err(pg_error)?;
        Ok(())
    }

    pub async fn repository_snapshot_files(
        &self,
        snapshot_id: &str,
    ) -> Result<Vec<RepositorySnapshotFile>, PersistenceError> {
        let client = self.client().await?;
        let rows = client.query(
            "SELECT snapshot_id,path,content_ref,content_hash,byte_length,media_type FROM repository_snapshot_files WHERE snapshot_id=$1 ORDER BY path",
            &[&snapshot_id],
        ).await.map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| RepositorySnapshotFile {
                snapshot_id: row.get(0),
                path: row.get(1),
                content_ref: row.get(2),
                content_hash: row.get(3),
                byte_length: row.get(4),
                media_type: row.get(5),
            })
            .collect())
    }

    pub async fn enqueue_controller_task(&self, task: &ControllerTask) -> Result<bool, PersistenceError> {
        let client = self.client().await?;
        let changed = client.execute(
            "INSERT INTO controller_tasks(id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,'pending',0,$9,NULL,NULL,$10,$10)
             ON CONFLICT(tenant_id,workspace_id,controller_id,deduplication_key) DO UPDATE SET status='pending',next_run_at=excluded.next_run_at,lease_until=NULL,last_error=NULL,updated_at=excluded.updated_at
             WHERE controller_tasks.status IN ('succeeded','failed')",
            &[&task.id,&task.tenant_id,&task.workspace_id,&task.controller_id,&task.resource_kind,&task.resource_id,&task.generation,&task.deduplication_key,&task.next_run_at,&task.created_at],
        ).await.map_err(pg_error)?;
        Ok(changed == 1)
    }

    pub async fn claim_due_controller_tasks(
        &self,
        now: &str,
        lease_until: &str,
        limit: usize,
    ) -> Result<Vec<ControllerTask>, PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let rows = transaction.query(
            "SELECT id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at
             FROM controller_tasks WHERE (status='pending' AND next_run_at <= $1) OR (status='running' AND lease_until <= $1)
             ORDER BY next_run_at,id LIMIT $2 FOR UPDATE SKIP LOCKED",
            &[&now, &(limit as i64)],
        ).await.map_err(pg_error)?;
        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get(0);
            transaction.execute("UPDATE controller_tasks SET status='running',attempts=attempts+1,lease_until=$1,updated_at=$2 WHERE id=$3", &[&lease_until,&now,&id]).await.map_err(pg_error)?;
            tasks.push(ControllerTask {
                id,
                tenant_id: row.get(1),
                workspace_id: row.get(2),
                controller_id: row.get(3),
                resource_kind: row.get(4),
                resource_id: row.get(5),
                generation: row.get(6),
                deduplication_key: row.get(7),
                status: "running".into(),
                attempts: row.get::<_, i64>(9) + 1,
                next_run_at: row.get(10),
                lease_until: Some(lease_until.into()),
                last_error: row.get(12),
                created_at: row.get(13),
                updated_at: now.into(),
            });
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(tasks)
    }

    pub async fn controller_task(&self, task_id: &str) -> Result<Option<ControllerTask>, PersistenceError> {
        let row = self
            .client()
            .await?
            .query_opt(
                "SELECT id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at FROM controller_tasks WHERE id=$1",
                &[&task_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(row.map(|row| ControllerTask {
            id: row.get(0),
            tenant_id: row.get(1),
            workspace_id: row.get(2),
            controller_id: row.get(3),
            resource_kind: row.get(4),
            resource_id: row.get(5),
            generation: row.get(6),
            deduplication_key: row.get(7),
            status: row.get(8),
            attempts: row.get(9),
            next_run_at: row.get(10),
            lease_until: row.get(11),
            last_error: row.get(12),
            created_at: row.get(13),
            updated_at: row.get(14),
        }))
    }

    pub async fn complete_controller_task(&self, task_id: &str, now: &str) -> Result<bool, PersistenceError> {
        Ok(self.client().await?.execute("UPDATE controller_tasks SET status='succeeded',lease_until=NULL,last_error=NULL,updated_at=$1 WHERE id=$2 AND status='running'", &[&now,&task_id]).await.map_err(pg_error)? == 1)
    }

    pub async fn fail_controller_task(
        &self,
        task_id: &str,
        retry: bool,
        error: &str,
        next_run_at: &str,
        now: &str,
    ) -> Result<bool, PersistenceError> {
        let status = if retry { "pending" } else { "failed" };
        Ok(self.client().await?.execute("UPDATE controller_tasks SET status=$1,lease_until=NULL,last_error=$2,next_run_at=$3,updated_at=$4 WHERE id=$5 AND status='running'", &[&status,&error,&next_run_at,&now,&task_id]).await.map_err(pg_error)? == 1)
    }

    pub async fn upsert_repository_sync_schedule(
        &self,
        schedule: &RepositorySyncSchedule,
    ) -> Result<(), PersistenceError> {
        let client = self.client().await?;
        client.execute(
            "INSERT INTO repository_sync_schedules(id,tenant_id,workspace_id,repository_id,ref_kind,ref_name,interval_seconds,enabled,next_run_at,created_at,updated_at)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)
             ON CONFLICT(tenant_id,workspace_id,repository_id,ref_kind,ref_name) DO UPDATE SET interval_seconds=excluded.interval_seconds,enabled=excluded.enabled,next_run_at=excluded.next_run_at,updated_at=excluded.updated_at",
            &[&schedule.id,&schedule.tenant_id,&schedule.workspace_id,&schedule.repository_id,&schedule.ref_kind,&schedule.ref_name,&schedule.interval_seconds,&schedule.enabled,&schedule.next_run_at,&schedule.created_at],
        ).await.map_err(pg_error)?;
        Ok(())
    }

    pub async fn due_repository_sync_schedules(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<RepositorySyncSchedule>, PersistenceError> {
        let rows = self.client().await?.query("SELECT id,tenant_id,workspace_id,repository_id,ref_kind,ref_name,interval_seconds,enabled,next_run_at,created_at,updated_at FROM repository_sync_schedules WHERE enabled=TRUE AND next_run_at <= $1 ORDER BY next_run_at,id LIMIT $2", &[&now,&(limit as i64)]).await.map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| RepositorySyncSchedule {
                id: row.get(0),
                tenant_id: row.get(1),
                workspace_id: row.get(2),
                repository_id: row.get(3),
                ref_kind: row.get(4),
                ref_name: row.get(5),
                interval_seconds: row.get(6),
                enabled: row.get(7),
                next_run_at: row.get(8),
                created_at: row.get(9),
                updated_at: row.get(10),
            })
            .collect())
    }

    pub async fn advance_repository_sync_schedule(
        &self,
        id: &str,
        next_run_at: &str,
        now: &str,
    ) -> Result<bool, PersistenceError> {
        Ok(self
            .client()
            .await?
            .execute(
                "UPDATE repository_sync_schedules SET next_run_at=$1,updated_at=$2 WHERE id=$3",
                &[&next_run_at, &now, &id],
            )
            .await
            .map_err(pg_error)?
            == 1)
    }

    pub async fn upsert_secret(&self, secret: &SecretRecord) -> Result<(), PersistenceError> {
        self.client().await?.execute("INSERT INTO secrets(id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(tenant_id,workspace_id,name) DO UPDATE SET encrypted_value=excluded.encrypted_value,key_version=excluded.key_version,updated_at=excluded.updated_at", &[&secret.id,&secret.tenant_id,&secret.workspace_id,&secret.name,&secret.encrypted_value,&secret.key_version,&secret.created_at,&secret.updated_at]).await.map_err(pg_error)?;
        Ok(())
    }

    pub async fn secret_by_id(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<SecretRecord>, PersistenceError> {
        let row = self.client().await?.query_opt("SELECT id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at FROM secrets WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3", &[&tenant_id,&workspace_id,&id]).await.map_err(pg_error)?;
        Ok(row.map(|row| SecretRecord {
            id: row.get(0),
            tenant_id: row.get(1),
            workspace_id: row.get(2),
            name: row.get(3),
            encrypted_value: row.get(4),
            key_version: row.get(5),
            created_at: row.get(6),
            updated_at: row.get(7),
        }))
    }

    pub async fn list_secrets(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<SecretRecord>, PersistenceError> {
        let rows = self.client().await?.query("SELECT id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at FROM secrets WHERE tenant_id=$1 AND workspace_id=$2 ORDER BY name,id", &[&tenant_id,&workspace_id]).await.map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| SecretRecord {
                id: row.get(0),
                tenant_id: row.get(1),
                workspace_id: row.get(2),
                name: row.get(3),
                encrypted_value: row.get(4),
                key_version: row.get(5),
                created_at: row.get(6),
                updated_at: row.get(7),
            })
            .collect())
    }

    pub async fn delete_secret(&self, tenant_id: &str, workspace_id: &str, id: &str) -> Result<bool, PersistenceError> {
        Ok(self
            .client()
            .await?
            .execute(
                "DELETE FROM secrets WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&tenant_id, &workspace_id, &id],
            )
            .await
            .map_err(pg_error)?
            == 1)
    }

    pub async fn upsert_integration(&self, integration: &IntegrationRecord) -> Result<(), PersistenceError> {
        self.client().await?.execute("INSERT INTO integrations(id,tenant_id,workspace_id,provider,display_name,secret_id,permissions,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(tenant_id,workspace_id,provider,display_name) DO UPDATE SET secret_id=excluded.secret_id,permissions=excluded.permissions,updated_at=excluded.updated_at", &[&integration.id,&integration.tenant_id,&integration.workspace_id,&integration.provider,&integration.display_name,&integration.secret_id,&integration.permissions,&integration.created_at,&integration.updated_at]).await.map_err(pg_error)?;
        Ok(())
    }

    pub async fn list_integrations(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<IntegrationRecord>, PersistenceError> {
        let rows = self.client().await?.query("SELECT id,tenant_id,workspace_id,provider,display_name,secret_id,permissions,created_at,updated_at FROM integrations WHERE tenant_id=$1 AND workspace_id=$2 ORDER BY display_name,id", &[&tenant_id,&workspace_id]).await.map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| IntegrationRecord {
                id: row.get(0),
                tenant_id: row.get(1),
                workspace_id: row.get(2),
                provider: row.get(3),
                display_name: row.get(4),
                secret_id: row.get(5),
                permissions: row.get(6),
                created_at: row.get(7),
                updated_at: row.get(8),
            })
            .collect())
    }

    pub async fn update_specification_code_binding_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &SpecificationCodeBinding,
        updated: &SpecificationCodeBinding,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError::new("specification code binding update scope does not match"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let changed = transaction
            .execute(
                "UPDATE specification_code_bindings
                 SET service_repository_id=$1,ref_kind=$2,ref_name=$3,updated_at=$4
                 WHERE id=$5 AND tenant_id=$6 AND workspace_id=$7 AND specification_id=$8
                   AND service_repository_id=$9 AND ref_kind=$10 AND ref_name=$11 AND updated_at=$12",
                &[
                    &updated.service_repository_id,
                    &specification_ref_kind_name(updated.ref_kind),
                    &updated.ref_name,
                    &updated.updated_at,
                    &expected.id,
                    &context.tenant_id,
                    &context.workspace_id,
                    &expected.specification_id,
                    &expected.service_repository_id,
                    &specification_ref_kind_name(expected.ref_kind),
                    &expected.ref_name,
                    &expected.updated_at,
                ],
            )
            .await
            .map_err(pg_error)?;
        if changed != 1 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn update_service_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &Service,
        updated: &Service,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError::new("service update scope does not match"));
        }

        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let changed = transaction
            .execute(
                "UPDATE services SET group_id=$1,name=$2,slug=$3,description=$4,updated_at=$5,archived=$6
                 WHERE id=$7 AND tenant_id=$8 AND workspace_id=$9 AND group_id IS NOT DISTINCT FROM $10
                   AND name=$11 AND slug=$12 AND description=$13 AND updated_at=$14 AND archived=$15",
                &[
                    &updated.group_id,
                    &updated.name,
                    &updated.slug,
                    &updated.description,
                    &updated.updated_at,
                    &updated.archived,
                    &expected.id,
                    &context.tenant_id,
                    &context.workspace_id,
                    &expected.group_id,
                    &expected.name,
                    &expected.slug,
                    &expected.description,
                    &expected.updated_at,
                    &expected.archived,
                ],
            )
            .await
            .map_err(pg_error)?;
        if changed != 1 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }

        transaction
            .execute(
                "DELETE FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3",
                &[&context.tenant_id, &context.workspace_id, &updated.id],
            )
            .await
            .map_err(pg_error)?;
        for tag_id in &updated.tag_ids {
            transaction
                .execute(
                    "INSERT INTO service_tags(tenant_id,workspace_id,service_id,tag_id) VALUES($1,$2,$3,$4)",
                    &[&context.tenant_id, &context.workspace_id, &updated.id, tag_id],
                )
                .await
                .map_err(pg_error)?;
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn update_code_source_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &CodeSource,
        updated: &CodeSource,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError::new("code source update scope does not match"));
        }

        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let changed = transaction
            .execute(
                "UPDATE code_sources SET source_type=$1,provider=$2,display_name=$3,canonical_key=$4,
                   root_locator=$5,execution_scope=$6,updated_at=$7,archived=$8
                 WHERE id=$9 AND tenant_id=$10 AND workspace_id=$11 AND source_type=$12 AND provider=$13
                   AND display_name=$14 AND canonical_key=$15 AND root_locator=$16 AND execution_scope=$17
                   AND updated_at=$18 AND archived=$19",
                &[
                    &source_type_name(updated.source_type),
                    &updated.provider,
                    &updated.display_name,
                    &updated.canonical_key,
                    &updated.root_locator,
                    &execution_scope_name(updated.execution_scope),
                    &updated.updated_at,
                    &updated.archived,
                    &expected.id,
                    &context.tenant_id,
                    &context.workspace_id,
                    &source_type_name(expected.source_type),
                    &expected.provider,
                    &expected.display_name,
                    &expected.canonical_key,
                    &expected.root_locator,
                    &execution_scope_name(expected.execution_scope),
                    &expected.updated_at,
                    &expected.archived,
                ],
            )
            .await
            .map_err(pg_error)?;
        if changed != 1 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn update_code_binding_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &ServiceCodeBinding,
        updated: &ServiceCodeBinding,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError::new("code binding update scope does not match"));
        }

        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        let changed = transaction
            .execute(
                "UPDATE service_code_bindings SET role=$1,entity_kind=$2,path=$3,revision_kind=$4,
                   revision=$5,updated_at=$6
                 WHERE id=$7 AND tenant_id=$8 AND workspace_id=$9 AND service_id=$10 AND source_id=$11
                   AND role=$12 AND entity_kind=$13 AND path=$14 AND revision_kind=$15
                   AND revision IS NOT DISTINCT FROM $16 AND updated_at=$17",
                &[
                    &binding_role_name(updated.role),
                    &entity_kind_name(updated.entity_kind),
                    &updated.path,
                    &revision_kind_name(updated.revision_kind),
                    &updated.revision,
                    &updated.updated_at,
                    &expected.id,
                    &context.tenant_id,
                    &context.workspace_id,
                    &expected.service_id,
                    &expected.source_id,
                    &binding_role_name(expected.role),
                    &entity_kind_name(expected.entity_kind),
                    &expected.path,
                    &revision_kind_name(expected.revision_kind),
                    &expected.revision,
                    &expected.updated_at,
                ],
            )
            .await
            .map_err(pg_error)?;
        if changed != 1 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit(&transaction, event).await?;
        }
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
    ) -> Result<Option<String>, PersistenceError> {
        let client = self.client().await?;
        let row = client.query_opt("SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL", &[&context.tenant_id, &context.workspace_id, &actor_id]).await.map_err(pg_error)?;
        Ok(row.map(|row| row.get(0)))
    }

    pub async fn workspace_members(
        &self,
        context: &WorkspaceContext,
    ) -> Result<Vec<WorkspaceMember>, PersistenceError> {
        let client = self.client().await?;
        let rows = client
            .query(
                "SELECT actor_id,role,revoked_at,created_at FROM workspace_members
                 WHERE tenant_id=$1 AND workspace_id=$2 ORDER BY actor_id",
                &[&context.tenant_id, &context.workspace_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| WorkspaceMember {
                actor_id: row.get(0),
                role: row.get(1),
                revoked_at: row.get(2),
                created_at: row.get(3),
            })
            .collect())
    }

    pub async fn create_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        invitation: &WorkspaceInvitation,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if invitation.tenant_id != context.tenant_id
            || invitation.workspace_id != context.workspace_id
            || invitation.email.trim().is_empty()
            || !matches!(invitation.role.as_str(), "admin" | "editor" | "viewer")
            || invitation.token_hash.is_empty()
        {
            return Err(PersistenceError::new("invalid workspace invitation"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        transaction
            .execute(
                "INSERT INTO workspace_invitations(id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at)
                 VALUES($1,$2,$3,$4,$5,$6,$7,NULL,NULL,$8,$9)",
                &[
                    &invitation.id,
                    &invitation.tenant_id,
                    &invitation.workspace_id,
                    &invitation.email.trim(),
                    &invitation.role,
                    &invitation.token_hash,
                    &invitation.expires_at,
                    &context.actor_id,
                    &invitation.created_at,
                ],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn workspace_invitations(
        &self,
        context: &WorkspaceContext,
    ) -> Result<Vec<WorkspaceInvitation>, PersistenceError> {
        let client = self.client().await?;
        let rows = client
            .query(
                "SELECT id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at
                 FROM workspace_invitations WHERE tenant_id=$1 AND workspace_id=$2 ORDER BY created_at DESC,id",
                &[&context.tenant_id, &context.workspace_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(rows.into_iter().map(|row| invitation_from_row(&row)).collect())
    }

    /// Remove only expired invitations that were never accepted or revoked.
    /// Accepted and revoked rows remain available for audit/history queries.
    pub async fn purge_expired_workspace_invitations(&self, now: &str) -> Result<u64, PersistenceError> {
        let client = self.client().await?;
        let changed = client
            .execute(
                "DELETE FROM workspace_invitations
                 WHERE expires_at <= $1 AND accepted_at IS NULL AND revoked_at IS NULL",
                &[&now],
            )
            .await
            .map_err(pg_error)?;
        Ok(changed)
    }

    pub async fn find_workspace_invitation(
        &self,
        tenant_id: &str,
        token_hash: &str,
    ) -> Result<Option<WorkspaceInvitation>, PersistenceError> {
        let client = self.client().await?;
        Ok(client
            .query_opt(
                "SELECT id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at
                 FROM workspace_invitations WHERE tenant_id=$1 AND token_hash=$2",
                &[&tenant_id, &token_hash],
            )
            .await
            .map_err(pg_error)?
            .map(|row| invitation_from_row(&row)))
    }

    pub async fn accept_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        token_hash: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<Option<WorkspaceMember>, PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let invitation = transaction
            .query_opt(
                "SELECT id,role,expires_at FROM workspace_invitations
                 WHERE tenant_id=$1 AND workspace_id=$2 AND token_hash=$3 AND accepted_at IS NULL AND revoked_at IS NULL FOR UPDATE",
                &[&context.tenant_id, &context.workspace_id, &token_hash],
            )
            .await
            .map_err(pg_error)?;
        let Some(invitation) = invitation else {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(None);
        };
        let invitation_id: String = invitation.get(0);
        let role: String = invitation.get(1);
        let expires_at: String = invitation.get(2);
        if expires_at.as_str() <= now {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(None);
        }
        transaction
            .execute(
                "INSERT INTO workspace_members(tenant_id,workspace_id,actor_id,role,revoked_at,created_at)
                 VALUES($1,$2,$3,$4,NULL,$5)
                 ON CONFLICT(tenant_id,workspace_id,actor_id) DO UPDATE SET role=excluded.role,revoked_at=NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id, &role, &now],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("UPDATE workspace_invitations SET accepted_at=$1 WHERE id=$2", &[&now, &invitation_id])
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(Some(WorkspaceMember {
            actor_id: context.actor_id.clone(),
            role,
            revoked_at: None,
            created_at: now.to_owned(),
        }))
    }

    pub async fn revoke_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        invitation_id: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        let changed = transaction
            .execute(
                "UPDATE workspace_invitations SET revoked_at=$1
                 WHERE id=$2 AND tenant_id=$3 AND workspace_id=$4 AND accepted_at IS NULL AND revoked_at IS NULL",
                &[&now, &invitation_id, &context.tenant_id, &context.workspace_id],
            )
            .await
            .map_err(pg_error)?;
        if changed == 0 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn workspaces_for_actor(
        &self,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<Vec<WorkspaceMembership>, PersistenceError> {
        let client = self.client().await?;
        let rows = client
            .query(
                "SELECT w.id,w.tenant_id,w.name,w.slug,w.created_at,m.role
                 FROM workspaces w
                 INNER JOIN workspace_members m ON m.tenant_id=w.tenant_id AND m.workspace_id=w.id
                 WHERE w.tenant_id=$1 AND m.actor_id=$2 AND m.revoked_at IS NULL
                 ORDER BY w.created_at,w.id",
                &[&tenant_id, &actor_id],
            )
            .await
            .map_err(pg_error)?;
        Ok(rows
            .into_iter()
            .map(|row| WorkspaceMembership {
                workspace: Workspace {
                    id: row.get(0),
                    tenant_id: row.get(1),
                    name: row.get(2),
                    slug: row.get(3),
                    created_at: row.get(4),
                },
                role: row.get(5),
            })
            .collect())
    }

    pub async fn create_workspace(
        &self,
        context: &WorkspaceContext,
        workspace: &Workspace,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace.tenant_id != context.tenant_id || workspace.id.is_empty() || workspace.name.trim().is_empty() {
            return Err(PersistenceError::new("invalid workspace metadata"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let role = transaction
            .query_opt(
                "SELECT role FROM workspace_members
                 WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        transaction
            .execute(
                "INSERT INTO workspaces(tenant_id,id,name,slug,created_at) VALUES($1,$2,$3,$4,$5)",
                &[&workspace.tenant_id, &workspace.id, &workspace.name.trim(), &workspace.slug, &workspace.created_at],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "INSERT INTO workspace_members(tenant_id,workspace_id,actor_id,role,created_at) VALUES($1,$2,$3,'owner',$4)",
                &[&workspace.tenant_id, &workspace.id, &context.actor_id, &workspace.created_at],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn update_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        name: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace_id != context.workspace_id || name.trim().is_empty() {
            return Err(PersistenceError::new("invalid workspace update"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        let changed = transaction
            .execute(
                "UPDATE workspaces SET name=$1 WHERE tenant_id=$2 AND id=$3",
                &[&name.trim(), &context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        if changed == 0 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn delete_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace_id != context.workspace_id || workspace_id == "default" {
            return Err(PersistenceError::new("workspace cannot be deleted"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if role.as_deref() != Some("owner") {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        transaction
            .execute(
                "DELETE FROM specification_revisions WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM specification_code_bindings WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM specifications WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_code_bindings WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_ref_tracks WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_repositories WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM repositories WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM code_sources WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM services WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute("DELETE FROM tags WHERE tenant_id=$1 AND workspace_id=$2", &[&context.tenant_id, &workspace_id])
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_groups WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM workspace_invitations WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2",
                &[&context.tenant_id, &workspace_id],
            )
            .await
            .map_err(pg_error)?;
        let changed = transaction
            .execute("DELETE FROM workspaces WHERE tenant_id=$1 AND id=$2", &[&context.tenant_id, &workspace_id])
            .await
            .map_err(pg_error)?;
        if changed == 0 {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        }
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn delete_group(
        &self,
        context: &WorkspaceContext,
        group_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM service_groups WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &group_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_tag(
        &self,
        context: &WorkspaceContext,
        tag_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2 AND tag_id=$3",
                &[&context.tenant_id, &context.workspace_id, &tag_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM tags WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &tag_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_service(
        &self,
        context: &WorkspaceContext,
        service_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction.execute("DELETE FROM specification_revisions WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id IN (SELECT id FROM specifications WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3)", &[&context.tenant_id, &context.workspace_id, &service_id]).await.map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM specifications WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3",
                &[&context.tenant_id, &context.workspace_id, &service_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_tags WHERE tenant_id=$1 AND workspace_id=$2 AND service_id=$3",
                &[&context.tenant_id, &context.workspace_id, &service_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM services WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &service_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_code_source(
        &self,
        context: &WorkspaceContext,
        source_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM code_sources WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &source_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_code_binding(
        &self,
        context: &WorkspaceContext,
        binding_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_code_bindings WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &binding_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_service_repository(
        &self,
        context: &WorkspaceContext,
        service_repository_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM specification_code_bindings WHERE tenant_id=$1 AND workspace_id=$2 AND service_repository_id=$3",
                &[&context.tenant_id, &context.workspace_id, &service_repository_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM service_repositories WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &service_repository_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_service_ref_track(
        &self,
        context: &WorkspaceContext,
        track_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM service_ref_tracks WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &track_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_specification_code_binding(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM specification_code_bindings WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id=$3",
                &[&context.tenant_id, &context.workspace_id, &specification_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn delete_specification(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "DELETE FROM specification_revisions WHERE tenant_id=$1 AND workspace_id=$2 AND specification_id=$3",
                &[&context.tenant_id, &context.workspace_id, &specification_id],
            )
            .await
            .map_err(pg_error)?;
        transaction
            .execute(
                "DELETE FROM specifications WHERE tenant_id=$1 AND workspace_id=$2 AND id=$3",
                &[&context.tenant_id, &context.workspace_id, &specification_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)
    }

    pub async fn set_workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        role: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if !matches!(role, "owner" | "admin" | "editor" | "viewer") {
            return Err(PersistenceError::new("invalid workspace member role"));
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let current_role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        if current_role.as_deref() == Some("owner") && role != "owner" {
            let row = transaction
                .query_one(
                    "SELECT count(*) FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND role='owner' AND revoked_at IS NULL",
                    &[&context.tenant_id, &context.workspace_id],
                )
                .await
                .map_err(pg_error)?;
            if row.get::<_, i64>(0) <= 1 {
                transaction.commit().await.map_err(pg_error)?;
                return Ok(false);
            }
        }
        let now = chrono::Utc::now().to_rfc3339();
        transaction
            .execute(
                "INSERT INTO workspace_members(tenant_id,workspace_id,actor_id,role,revoked_at,created_at)
                 VALUES($1,$2,$3,$4,NULL,$5) ON CONFLICT(tenant_id,workspace_id,actor_id)
                 DO UPDATE SET role=excluded.role,revoked_at=NULL",
                &[&context.tenant_id, &context.workspace_id, &actor_id, &role, &now],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn revoke_workspace_member_with_audit(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        let current_role = transaction
            .query_opt(
                "SELECT role FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3 AND revoked_at IS NULL",
                &[&context.tenant_id, &context.workspace_id, &actor_id],
            )
            .await
            .map_err(pg_error)?
            .map(|row| row.get::<_, String>(0));
        let Some(current_role) = current_role else {
            transaction.commit().await.map_err(pg_error)?;
            return Ok(false);
        };
        if current_role == "owner" {
            let row = transaction
                .query_one(
                    "SELECT count(*) FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2 AND role='owner' AND revoked_at IS NULL",
                    &[&context.tenant_id, &context.workspace_id],
                )
                .await
                .map_err(pg_error)?;
            if row.get::<_, i64>(0) <= 1 {
                transaction.commit().await.map_err(pg_error)?;
                return Ok(false);
            }
        }
        transaction
            .execute(
                "UPDATE workspace_members SET revoked_at=$1 WHERE tenant_id=$2 AND workspace_id=$3 AND actor_id=$4 AND revoked_at IS NULL",
                &[&chrono::Utc::now().to_rfc3339(), &context.tenant_id, &context.workspace_id, &actor_id],
            )
            .await
            .map_err(pg_error)?;
        insert_audit(&transaction, audit).await?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(true)
    }

    pub async fn authorize_workspace(&self, context: &WorkspaceContext) -> Result<bool, PersistenceError> {
        if self.workspace_member_role(context, &context.actor_id).await?.is_some() {
            return Ok(true);
        }
        if context.workspace_id != "default" {
            return Ok(false);
        }
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        lock_workspace(&transaction, context).await?;
        transaction
            .execute(
                "INSERT INTO workspaces(tenant_id,id,name,slug,created_at) VALUES($1,$2,'默认工作区','default',$3)
                 ON CONFLICT(tenant_id,id) DO NOTHING",
                &[&context.tenant_id, &context.workspace_id, &chrono::Utc::now().to_rfc3339()],
            )
            .await
            .map_err(pg_error)?;
        transaction.execute(
            "INSERT INTO workspace_members(tenant_id,workspace_id,actor_id,role,created_at)
             SELECT $1,$2,$3,'owner',now() WHERE NOT EXISTS (SELECT 1 FROM workspace_members WHERE tenant_id=$1 AND workspace_id=$2)",
            &[&context.tenant_id, &context.workspace_id, &context.actor_id],
        ).await.map_err(pg_error)?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(self.workspace_member_role(context, &context.actor_id).await?.is_some())
    }

    pub async fn consume_rate_limit(
        &self,
        context: &WorkspaceContext,
        now: i64,
        window_seconds: i64,
        limit: u32,
    ) -> Result<bool, PersistenceError> {
        let mut client = self.client().await?;
        let transaction = client.transaction().await.map_err(pg_error)?;
        transaction.execute(
            "INSERT INTO api_rate_limits(tenant_id,workspace_id,actor_id,window_start,request_count) VALUES($1,$2,$3,$4,1)
             ON CONFLICT(tenant_id,workspace_id,actor_id) DO UPDATE SET
               window_start=CASE WHEN EXCLUDED.window_start >= api_rate_limits.window_start + $5 THEN EXCLUDED.window_start ELSE api_rate_limits.window_start END,
               request_count=CASE WHEN EXCLUDED.window_start >= api_rate_limits.window_start + $5 THEN 1 ELSE api_rate_limits.request_count+1 END",
            &[&context.tenant_id, &context.workspace_id, &context.actor_id, &now, &window_seconds],
        ).await.map_err(pg_error)?;
        let row = transaction
            .query_one(
                "SELECT request_count FROM api_rate_limits WHERE tenant_id=$1 AND workspace_id=$2 AND actor_id=$3",
                &[&context.tenant_id, &context.workspace_id, &context.actor_id],
            )
            .await
            .map_err(pg_error)?;
        transaction.commit().await.map_err(pg_error)?;
        Ok(row.get::<_, i64>(0) <= i64::from(limit))
    }

    async fn client(&self) -> Result<deadpool_postgres::Client, PersistenceError> {
        self.pool.get().await.map_err(|error| PersistenceError::new(format!("PostgreSQL connection: {error}")))
    }
}

fn postgres_tls_required() -> bool {
    std::env::var("SPECX_POSTGRES_REQUIRE_TLS")
        .ok()
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn postgres_tls_connector(url: &str) -> Result<tokio_postgres_rustls::MakeRustlsConnect, PersistenceError> {
    let sslmode = url_query_value(url, "sslmode").unwrap_or_else(|| "prefer".to_owned()).to_ascii_lowercase();
    let verify_server = matches!(sslmode.as_str(), "verify-ca" | "verify-full" | "verify_identity" | "verify-identity")
        || std::env::var("SPECX_POSTGRES_VERIFY_SERVER_CERT")
            .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
    let verify_hostname = matches!(sslmode.as_str(), "verify-full" | "verify_identity" | "verify-identity");
    let ca_path = std::env::var("SPECX_POSTGRES_CA_FILE").ok().or_else(|| url_query_value(url, "sslrootcert"));
    let cert_path = std::env::var("SPECX_POSTGRES_CLIENT_CERT").ok().or_else(|| url_query_value(url, "sslcert"));
    let key_path = std::env::var("SPECX_POSTGRES_CLIENT_KEY").ok().or_else(|| url_query_value(url, "sslkey"));

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let builder = if verify_server {
        let roots = postgres_root_cert_store(ca_path.as_deref())?;
        if verify_hostname {
            rustls::ClientConfig::builder().with_root_certificates(roots)
        } else {
            rustls::ClientConfig::builder().dangerous().with_custom_certificate_verifier(Arc::new(
                PostgresCaOnlyVerifier { provider: provider.clone(), roots: Arc::new(roots) },
            ))
        }
    } else {
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PostgresNoVerifier { provider: provider.clone() }))
    };
    let client_config = if cert_path.is_some() || key_path.is_some() {
        let (Some(cert_path), Some(key_path)) = (cert_path.as_deref(), key_path.as_deref()) else {
            return Err(PersistenceError::new(
                "PostgreSQL client certificate requires both SPECX_POSTGRES_CLIENT_CERT and SPECX_POSTGRES_CLIENT_KEY",
            ));
        };
        let certs = read_postgres_pem_certs(cert_path)?;
        let key = read_postgres_private_key(key_path)?;
        let signing_key = provider
            .key_provider
            .load_private_key(key)
            .map_err(|error| PersistenceError::new(format!("PostgreSQL client private key is invalid: {error}")))?;
        let certified_key = rustls::sign::CertifiedKey::new(certs, signing_key);
        let resolver = Arc::new(rustls::sign::SingleCertAndKey::from(certified_key));
        // Rebuild the verifier branch with the client certificate resolver.
        builder.with_client_cert_resolver(resolver)
    } else {
        builder.with_no_client_auth()
    };
    Ok(tokio_postgres_rustls::MakeRustlsConnect::new(client_config))
}

fn url_query_value(url: &str, key: &str) -> Option<String> {
    url.split_once('?')?.1.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        name.eq_ignore_ascii_case(key).then(|| value.to_owned())
    })
}

fn postgres_config_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let query = query
        .split('&')
        .filter(|part| {
            part.split_once('=').is_none_or(|(name, _)| {
                !matches!(name.to_ascii_lowercase().as_str(), "sslmode" | "sslrootcert" | "sslcert" | "sslkey")
            })
        })
        .collect::<Vec<_>>();
    if query.is_empty() { base.to_owned() } else { format!("{base}?{}", query.join("&")) }
}

fn postgres_ssl_mode(url: &str) -> Result<SslMode, PersistenceError> {
    let mode = url_query_value(url, "sslmode").unwrap_or_else(|| "prefer".to_owned()).to_ascii_lowercase();
    match mode.as_str() {
        "disable" => Ok(SslMode::Disable),
        "prefer" => Ok(SslMode::Prefer),
        "require" | "verify-ca" | "verify-full" | "verify_identity" | "verify-identity" => Ok(SslMode::Require),
        _ => Err(PersistenceError::new(format!("unsupported PostgreSQL sslmode: {mode}"))),
    }
}

fn postgres_root_cert_store(ca_path: Option<&str>) -> Result<rustls::RootCertStore, PersistenceError> {
    let mut roots = rustls::RootCertStore::empty();
    if let Some(path) = ca_path {
        let certs = read_postgres_pem_certs(path)?;
        let (valid, _) = roots.add_parsable_certificates(certs);
        if valid == 0 {
            return Err(PersistenceError::new(format!("PostgreSQL CA file contains no valid certificates: {path}")));
        }
    } else {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    Ok(roots)
}

fn read_postgres_pem_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, PersistenceError> {
    let file = std::fs::File::open(path).map_err(PersistenceError::from)?;
    rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| PersistenceError::new(format!("PostgreSQL certificate PEM is invalid: {error}")))
}

fn read_postgres_private_key(path: &str) -> Result<PrivateKeyDer<'static>, PersistenceError> {
    let file = std::fs::File::open(path).map_err(PersistenceError::from)?;
    rustls_pemfile::private_key(&mut BufReader::new(file))
        .map_err(|error| PersistenceError::new(format!("PostgreSQL private key PEM is invalid: {error}")))?
        .ok_or_else(|| PersistenceError::new(format!("PostgreSQL private key is empty: {path}")))
}

#[derive(Debug)]
struct PostgresNoVerifier {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PostgresNoVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

#[derive(Debug)]
struct PostgresCaOnlyVerifier {
    provider: Arc<CryptoProvider>,
    roots: Arc<rustls::RootCertStore>,
}

impl ServerCertVerifier for PostgresCaOnlyVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let cert = ParsedCertificate::try_from(end_entity)?;
        verify_server_cert_signed_by_trust_anchor(
            &cert,
            &self.roots,
            intermediates,
            now,
            self.provider.signature_verification_algorithms.all,
        )?;
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

fn in_scope(context: &WorkspaceContext, tenant_id: &str, workspace_id: &str) -> bool {
    tenant_id == context.tenant_id && workspace_id == context.workspace_id
}
fn ensure_scope(changed: u64, entity: &str) -> Result<(), PersistenceError> {
    if changed == 1 { Ok(()) } else { Err(PersistenceError::new(format!("{entity} id belongs to a different scope"))) }
}
fn pg_error(error: tokio_postgres::Error) -> PersistenceError {
    PersistenceError::new(error.to_string())
}
fn kind_name(kind: SpecificationKind) -> &'static str {
    match kind {
        SpecificationKind::Markdown => "markdown",
    }
}
fn parse_kind(kind: &str) -> Result<SpecificationKind, PersistenceError> {
    match kind {
        "markdown" => Ok(SpecificationKind::Markdown),
        _ => Err(PersistenceError::new(format!("unsupported specification kind: {kind}"))),
    }
}
fn source_type_name(value: CodeSourceType) -> &'static str {
    match value {
        CodeSourceType::Git => "git",
        CodeSourceType::LocalFilesystem => "local_filesystem",
    }
}
fn parse_source_type(value: &str) -> Result<CodeSourceType, PersistenceError> {
    match value {
        "git" => Ok(CodeSourceType::Git),
        "local_filesystem" => Ok(CodeSourceType::LocalFilesystem),
        _ => Err(PersistenceError::new(format!("unsupported code source type: {value}"))),
    }
}
fn execution_scope_name(value: CodeExecutionScope) -> &'static str {
    match value {
        CodeExecutionScope::Server => "server",
        CodeExecutionScope::DesktopHost => "desktop_host",
    }
}
fn parse_execution_scope(value: &str) -> Result<CodeExecutionScope, PersistenceError> {
    match value {
        "server" => Ok(CodeExecutionScope::Server),
        "desktop_host" => Ok(CodeExecutionScope::DesktopHost),
        _ => Err(PersistenceError::new(format!("unsupported code execution scope: {value}"))),
    }
}
fn ref_kind_name(value: ServiceRefKind) -> &'static str {
    match value {
        ServiceRefKind::DefaultBranch => "default_branch",
        ServiceRefKind::Branch => "branch",
        ServiceRefKind::Tag => "tag",
        ServiceRefKind::Commit => "commit",
    }
}

fn specification_ref_kind_name(value: SpecificationRefKind) -> &'static str {
    match value {
        SpecificationRefKind::Branch => "branch",
        SpecificationRefKind::Tag => "tag",
        SpecificationRefKind::Commit => "commit",
    }
}

fn parse_specification_ref_kind(value: &str) -> Result<SpecificationRefKind, PersistenceError> {
    match value {
        "branch" => Ok(SpecificationRefKind::Branch),
        "tag" => Ok(SpecificationRefKind::Tag),
        "commit" => Ok(SpecificationRefKind::Commit),
        _ => Err(PersistenceError::new("invalid specification ref kind")),
    }
}
fn parse_ref_kind(value: &str) -> Result<ServiceRefKind, PersistenceError> {
    match value {
        "default_branch" => Ok(ServiceRefKind::DefaultBranch),
        "branch" => Ok(ServiceRefKind::Branch),
        "tag" => Ok(ServiceRefKind::Tag),
        "commit" => Ok(ServiceRefKind::Commit),
        _ => Err(PersistenceError::new(format!("unsupported service ref kind: {value}"))),
    }
}
fn binding_role_name(value: CodeBindingRole) -> &'static str {
    match value {
        CodeBindingRole::Primary => "primary",
        CodeBindingRole::Additional => "additional",
    }
}
fn parse_binding_role(value: &str) -> Result<CodeBindingRole, PersistenceError> {
    match value {
        "primary" => Ok(CodeBindingRole::Primary),
        "additional" => Ok(CodeBindingRole::Additional),
        _ => Err(PersistenceError::new(format!("unsupported code binding role: {value}"))),
    }
}
fn entity_kind_name(value: CodeEntityKind) -> &'static str {
    match value {
        CodeEntityKind::RepositoryRoot => "repository_root",
        CodeEntityKind::Directory => "directory",
        CodeEntityKind::File => "file",
        CodeEntityKind::Pattern => "pattern",
    }
}
fn parse_entity_kind(value: &str) -> Result<CodeEntityKind, PersistenceError> {
    match value {
        "repository_root" => Ok(CodeEntityKind::RepositoryRoot),
        "directory" => Ok(CodeEntityKind::Directory),
        "file" => Ok(CodeEntityKind::File),
        "pattern" => Ok(CodeEntityKind::Pattern),
        _ => Err(PersistenceError::new(format!("unsupported code entity kind: {value}"))),
    }
}
fn revision_kind_name(value: CodeRevisionKind) -> &'static str {
    match value {
        CodeRevisionKind::DefaultBranch => "default_branch",
        CodeRevisionKind::Branch => "branch",
        CodeRevisionKind::Tag => "tag",
        CodeRevisionKind::Commit => "commit",
        CodeRevisionKind::WorkingTree => "working_tree",
    }
}
fn parse_revision_kind(value: &str) -> Result<CodeRevisionKind, PersistenceError> {
    match value {
        "default_branch" => Ok(CodeRevisionKind::DefaultBranch),
        "branch" => Ok(CodeRevisionKind::Branch),
        "tag" => Ok(CodeRevisionKind::Tag),
        "commit" => Ok(CodeRevisionKind::Commit),
        "working_tree" => Ok(CodeRevisionKind::WorkingTree),
        _ => Err(PersistenceError::new(format!("unsupported code revision kind: {value}"))),
    }
}
fn content_hash(content: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(content.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}
fn group_from_row(row: &Row) -> ServiceGroup {
    ServiceGroup {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        parent_id: row.get(3),
        name: row.get(4),
        position: row.get(5),
    }
}
fn tag_from_row(row: &Row) -> Tag {
    Tag { id: row.get(0), tenant_id: row.get(1), workspace_id: row.get(2), name: row.get(3), color: row.get(4) }
}
fn service_from_row(row: &Row) -> Service {
    Service {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        group_id: row.get(3),
        name: row.get(4),
        slug: row.get(5),
        description: row.get(6),
        tag_ids: vec![],
        updated_at: row.get(7),
        archived: row.get(8),
    }
}
fn specification_from_row(row: &Row) -> Result<Specification, PersistenceError> {
    Ok(Specification {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        service_id: row.get(3),
        title: row.get(4),
        kind: parse_kind(&row.get::<_, String>(5))?,
        content: row.get(6),
        version: row.get(7),
        content_hash: row.get(8),
        updated_at: row.get(9),
    })
}
fn code_source_from_row(row: &Row) -> Result<CodeSource, PersistenceError> {
    Ok(CodeSource {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        source_type: parse_source_type(&row.get::<_, String>(3))?,
        provider: row.get(4),
        display_name: row.get(5),
        canonical_key: row.get(6),
        root_locator: row.get(7),
        execution_scope: parse_execution_scope(&row.get::<_, String>(8))?,
        created_at: row.get(9),
        updated_at: row.get(10),
        archived: row.get(11),
    })
}
fn code_binding_from_row(row: &Row) -> Result<ServiceCodeBinding, PersistenceError> {
    Ok(ServiceCodeBinding {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        service_id: row.get(3),
        source_id: row.get(4),
        role: parse_binding_role(&row.get::<_, String>(5))?,
        entity_kind: parse_entity_kind(&row.get::<_, String>(6))?,
        path: row.get(7),
        revision_kind: parse_revision_kind(&row.get::<_, String>(8))?,
        revision: row.get(9),
        created_at: row.get(10),
        updated_at: row.get(11),
    })
}

fn repository_from_row(row: &Row) -> Repository {
    Repository {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        provider: row.get(3),
        display_name: row.get(4),
        canonical_url: row.get(5),
        created_at: row.get(6),
        updated_at: row.get(7),
        archived: row.get(8),
    }
}

fn repository_snapshot_from_row(row: &Row) -> RepositorySnapshot {
    RepositorySnapshot {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        repository_id: row.get(3),
        source_ref: row.get(4),
        resolved_commit: row.get(5),
        manifest_hash: row.get(6),
        content_root_ref: row.get(7),
        fetched_at: row.get(8),
        provider_version: row.get(9),
        created_at: row.get(10),
    }
}

fn service_repository_from_row(row: &Row) -> ServiceRepository {
    ServiceRepository {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        service_id: row.get(3),
        repository_id: row.get(4),
        created_at: row.get(5),
        updated_at: row.get(6),
    }
}

fn ref_track_from_row(row: &Row) -> Result<ServiceRefTrack, PersistenceError> {
    Ok(ServiceRefTrack {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        service_repository_id: row.get(3),
        ref_kind: parse_ref_kind(&row.get::<_, String>(4))?,
        ref_name: row.get(5),
        created_at: row.get(6),
        updated_at: row.get(7),
    })
}

fn specification_code_binding_from_row(row: &Row) -> Result<SpecificationCodeBinding, PersistenceError> {
    Ok(SpecificationCodeBinding {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        specification_id: row.get(3),
        service_repository_id: row.get(4),
        path: row.get(5),
        ref_kind: parse_specification_ref_kind(&row.get::<_, String>(6))?,
        ref_name: row.get(7),
        created_at: row.get(8),
        updated_at: row.get(9),
    })
}

fn invitation_from_row(row: &Row) -> WorkspaceInvitation {
    WorkspaceInvitation {
        id: row.get(0),
        tenant_id: row.get(1),
        workspace_id: row.get(2),
        email: row.get(3),
        role: row.get(4),
        token_hash: row.get(5),
        expires_at: row.get(6),
        accepted_at: row.get(7),
        revoked_at: row.get(8),
        created_by: row.get(9),
        created_at: row.get(10),
    }
}

async fn insert_audit(transaction: &Transaction<'_>, event: &AuditEvent) -> Result<(), PersistenceError> {
    transaction.execute(
        "INSERT INTO audit_events(tenant_id,workspace_id,actor_id,request_id,action,resource_type,resource_id,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        &[&event.tenant_id, &event.workspace_id, &event.actor_id, &event.request_id, &event.action, &event.resource_type, &event.resource_id, &event.created_at],
    ).await.map(|_| ()).map_err(pg_error)
}

async fn lock_workspace(transaction: &Transaction<'_>, context: &WorkspaceContext) -> Result<(), PersistenceError> {
    let mut digest = Sha256::new();
    digest.update(context.tenant_id.as_bytes());
    digest.update([0]);
    digest.update(context.workspace_id.as_bytes());
    let bytes = digest.finalize();
    let key = i64::from_be_bytes(bytes[..8].try_into().expect("SHA-256 prefix has eight bytes"));
    transaction.query_one("SELECT pg_advisory_xact_lock($1)", &[&key]).await.map_err(pg_error)?;
    Ok(())
}

fn backup_timeout() -> Duration {
    let seconds = std::env::var("SPECX_BACKUP_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(300);
    Duration::from_secs(seconds)
}

fn create_pg_dump(
    database_url: &str,
    directory: &Path,
    prefix: &str,
    retention: usize,
    dump_bin: &str,
    timeout: Duration,
) -> Result<BackupArtifact, PersistenceError> {
    if retention == 0
        || prefix.is_empty()
        || prefix.len() > 160
        || !prefix.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(PersistenceError::new("invalid PostgreSQL backup prefix or retention"));
    }
    fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| PersistenceError::new(error.to_string()))?
        .as_nanos();
    let destination = directory.join(format!("{prefix}-{stamp}.dump"));
    let temporary = directory.join(format!(".{prefix}-{stamp}.tmp"));
    let result = (|| {
        let args = vec![
            OsString::from("--format=custom"),
            OsString::from("--no-owner"),
            OsString::from("--file"),
            temporary.as_os_str().to_owned(),
            OsString::from(database_url),
        ];
        run_external_command(dump_bin, &args, timeout)?;
        let metadata = fs::symlink_metadata(&temporary)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() == 0 {
            return Err(PersistenceError::new("pg_dump did not produce a valid snapshot"));
        }
        if let Some(key) = current_backup_key()? {
            let plaintext = fs::read(&temporary)?;
            let cipher =
                Aes256Gcm::new_from_slice(&key.key).map_err(|error| PersistenceError::new(error.to_string()))?;
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            let ciphertext = cipher
                .encrypt(&nonce, plaintext.as_ref())
                .map_err(|error| PersistenceError::new(format!("backup encryption failed: {error}")))?;
            // SPECXPG2 carries the key id so operators can rotate keys without
            // re-encrypting every existing snapshot in one maintenance window.
            if key.id.is_empty()
                || key.id.len() > u8::MAX as usize
                || !key.id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(PersistenceError::new(
                    "backup encryption key id must contain only ASCII letters, numbers, '-' or '_'",
                ));
            }
            let mut encrypted = b"SPECXPG2".to_vec();
            encrypted.push(key.id.len() as u8);
            encrypted.extend_from_slice(key.id.as_bytes());
            encrypted.extend_from_slice(&nonce);
            encrypted.extend_from_slice(&ciphertext);
            fs::write(&temporary, encrypted)?;
        }
        let metadata = fs::symlink_metadata(&temporary)?;
        let bytes = fs::read(&temporary)?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        fs::rename(&temporary, &destination)?;
        let mut snapshots = fs::read_dir(directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                    name.starts_with(prefix)
                        && name.ends_with(".dump")
                        && fs::symlink_metadata(path)
                            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
                })
            })
            .collect::<Vec<_>>();
        snapshots.sort();
        while snapshots.len() > retention {
            let old = snapshots.remove(0);
            fs::remove_file(old)?;
        }
        Ok(BackupArtifact { path: destination.clone(), sha256, bytes: metadata.len() })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        let _ = fs::remove_file(&destination);
    }
    result
}

fn restore_pg_dump(
    database_url: &str,
    source: &Path,
    restore_bin: &str,
    timeout: Duration,
) -> Result<(), PersistenceError> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || source.extension().and_then(|value| value.to_str()) != Some("dump")
    {
        return Err(PersistenceError::new("invalid PostgreSQL backup source"));
    }
    let encrypted = fs::read(source)?;
    let restore_source = if encrypted.starts_with(b"SPECXPG1") || encrypted.starts_with(b"SPECXPG2") {
        let keys = backup_keys()?.ok_or_else(|| {
            PersistenceError::new(
                "encrypted backup requires SPECX_BACKUP_ENCRYPTION_KEY or SPECX_BACKUP_ENCRYPTION_KEYS",
            )
        })?;
        let (key_candidates, nonce_offset) = if encrypted.starts_with(b"SPECXPG2") {
            if encrypted.len() <= 9 {
                return Err(PersistenceError::new("encrypted PostgreSQL backup is truncated"));
            }
            let id_len = encrypted[8] as usize;
            let id_end = 9usize.saturating_add(id_len);
            if id_len == 0 || encrypted.len() <= id_end + 12 {
                return Err(PersistenceError::new("encrypted PostgreSQL backup has an invalid key id"));
            }
            let key_id = std::str::from_utf8(&encrypted[9..id_end])
                .map_err(|_| PersistenceError::new("encrypted PostgreSQL backup key id is not valid UTF-8"))?;
            let key = keys.iter().find(|candidate| candidate.id == key_id).ok_or_else(|| {
                PersistenceError::new(format!("backup encryption key id '{key_id}' is not configured"))
            })?;
            (vec![key], id_end)
        } else {
            if encrypted.len() <= 8 + 12 {
                return Err(PersistenceError::new("encrypted PostgreSQL backup is truncated"));
            }
            (keys.iter().collect::<Vec<_>>(), 8)
        };
        let nonce_end = nonce_offset + 12;
        let nonce = Nonce::from_slice(&encrypted[nonce_offset..nonce_end]);
        let mut plaintext = None;
        for key in key_candidates {
            let cipher =
                Aes256Gcm::new_from_slice(&key.key).map_err(|error| PersistenceError::new(error.to_string()))?;
            if let Ok(value) = cipher.decrypt(nonce, &encrypted[nonce_end..]) {
                plaintext = Some(value);
                break;
            }
        }
        let plaintext = plaintext.ok_or_else(|| PersistenceError::new("backup decryption failed"))?;
        let temporary = source.with_file_name(format!(
            ".{}.restore-{}",
            source.file_name().and_then(|value| value.to_str()).unwrap_or("snapshot"),
            std::process::id()
        ));
        fs::write(&temporary, plaintext)?;
        temporary
    } else {
        if encryption_required() {
            return Err(PersistenceError::new("unencrypted PostgreSQL backup rejected because encryption is required"));
        }
        source.to_path_buf()
    };
    let args = vec![
        OsString::from("--clean"),
        OsString::from("--if-exists"),
        OsString::from("--no-owner"),
        OsString::from("--single-transaction"),
        OsString::from("--dbname"),
        OsString::from(database_url),
        restore_source.as_os_str().to_owned(),
    ];
    let result = run_external_command(restore_bin, &args, timeout);
    if restore_source != source {
        let _ = fs::remove_file(&restore_source);
    }
    result
}

fn encryption_required() -> bool {
    std::env::var("SPECX_BACKUP_REQUIRE_ENCRYPTION")
        .ok()
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackupKey {
    id: String,
    key: [u8; 32],
}

fn backup_keys() -> Result<Option<Vec<BackupKey>>, PersistenceError> {
    let mut entries = Vec::new();
    if let Some(value) = std::env::var("SPECX_BACKUP_ENCRYPTION_KEYS").ok().filter(|value| !value.trim().is_empty()) {
        for entry in value.split(',').map(str::trim).filter(|entry| !entry.is_empty()) {
            let (id, encoded) = entry.split_once('=').ok_or_else(|| {
                PersistenceError::new("SPECX_BACKUP_ENCRYPTION_KEYS entries must use id=base64 format")
            })?;
            entries.push(parse_backup_key(id.trim(), encoded.trim())?);
        }
    }
    if let Some(encoded) = std::env::var("SPECX_BACKUP_ENCRYPTION_KEY").ok().filter(|value| !value.trim().is_empty()) {
        // The legacy variable remains supported as the default key. If a
        // key-ring entry already uses that id, the explicit key wins so a
        // staged rotation can be rolled out one process at a time.
        let legacy = parse_backup_key(
            &std::env::var("SPECX_BACKUP_ENCRYPTION_KEY_ID").unwrap_or_else(|_| "default".to_owned()),
            encoded.trim(),
        )?;
        if let Some(existing) = entries.iter_mut().find(|entry| entry.id == legacy.id) {
            *existing = legacy;
        } else {
            entries.push(legacy);
        }
    }
    if entries.is_empty() {
        if encryption_required() {
            return Err(PersistenceError::new(
                "SPECX_BACKUP_ENCRYPTION_KEY or SPECX_BACKUP_ENCRYPTION_KEYS is required",
            ));
        }
        return Ok(None);
    }
    Ok(Some(entries))
}

fn current_backup_key() -> Result<Option<BackupKey>, PersistenceError> {
    let Some(keys) = backup_keys()? else { return Ok(None) };
    let requested_id = std::env::var("SPECX_BACKUP_ENCRYPTION_KEY_ID").ok().filter(|value| !value.trim().is_empty());
    let key = select_current_backup_key(&keys, requested_id.as_deref())?;
    Ok(Some(key))
}

fn select_current_backup_key(keys: &[BackupKey], requested_id: Option<&str>) -> Result<BackupKey, PersistenceError> {
    requested_id
        .and_then(|id| keys.iter().find(|key| key.id == id.trim()))
        .or_else(|| keys.first())
        .cloned()
        .ok_or_else(|| PersistenceError::new("no usable backup encryption key is configured"))
}

fn parse_backup_key(id: &str, encoded: &str) -> Result<BackupKey, PersistenceError> {
    if id.is_empty()
        || id.len() > u8::MAX as usize
        || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(PersistenceError::new(
            "backup encryption key id must contain only ASCII letters, numbers, '-' or '_'",
        ));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|error| PersistenceError::new(format!("invalid backup encryption key: {error}")))?;
    let key: [u8; 32] =
        decoded.try_into().map_err(|_| PersistenceError::new("backup encryption key must decode to 32 bytes"))?;
    Ok(BackupKey { id: id.to_owned(), key })
}

fn run_external_command(program: &str, args: &[OsString], timeout: Duration) -> Result<(), PersistenceError> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| PersistenceError::new(format!("failed to start {program}: {error}")))?;
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| PersistenceError::new(error.to_string()))? {
            if status.success() {
                return Ok(());
            }
            let output = child.wait_with_output().map_err(|error| PersistenceError::new(error.to_string()))?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim().chars().take(512).collect::<String>();
            return Err(PersistenceError::new(format!("{program} exited with {status}: {detail}")));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PersistenceError::new(format!("{program} timed out after {} seconds", timeout.as_secs())));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_connector_requires_a_valid_configured_ca() {
        let directory = tempfile::tempdir().unwrap();
        let ca = directory.path().join("invalid-ca.pem");
        fs::write(&ca, "not-a-certificate").unwrap();
        unsafe {
            std::env::set_var("SPECX_POSTGRES_CA_FILE", &ca);
        }
        let result = postgres_tls_connector("postgres://localhost/specx?sslmode=verify-ca");
        unsafe {
            std::env::remove_var("SPECX_POSTGRES_CA_FILE");
        }
        let error = match result {
            Ok(_) => panic!("invalid CA unexpectedly accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("CA file"));
    }

    #[test]
    fn tls_connector_rejects_a_partial_client_certificate_pair() {
        let directory = tempfile::tempdir().unwrap();
        let cert = directory.path().join("client.crt");
        fs::write(&cert, "not-a-certificate").unwrap();
        unsafe {
            std::env::set_var("SPECX_POSTGRES_CLIENT_CERT", &cert);
            std::env::remove_var("SPECX_POSTGRES_CLIENT_KEY");
        }
        let result = postgres_tls_connector("postgres://localhost/specx?sslmode=require");
        unsafe {
            std::env::remove_var("SPECX_POSTGRES_CLIENT_CERT");
        }
        let error = match result {
            Ok(_) => panic!("partial client certificate unexpectedly accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("requires both"));
    }

    #[test]
    fn backup_key_ring_selects_the_rotation_target_and_validates_ids() {
        let old = parse_backup_key("old", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        let new = parse_backup_key("new", "//////////////////////////////////////////8=").unwrap();
        assert_eq!(select_current_backup_key(&[old.clone(), new.clone()], Some("new")).unwrap(), new);
        assert_eq!(select_current_backup_key(&[old.clone(), new], None).unwrap(), old);
        assert!(parse_backup_key("bad/id", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").is_err());
    }
}
