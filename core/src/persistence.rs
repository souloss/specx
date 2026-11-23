//! Durable storage for the specX domain facts.
//!
//! This first adapter intentionally uses SQLite so the core can be exercised
//! without a running service. The repository shape and scoped queries are the
//! same contract a PostgreSQL adapter must implement for the production API.

use crate::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Core, Id,
    Repository, RepositorySnapshot, Service, ServiceCodeBinding, ServiceGroup, ServiceRefKind, ServiceRefTrack,
    ServiceRepository, Specification, SpecificationCodeBinding, SpecificationKind, SpecificationRefKind, Tag,
    Workspace, WorkspaceContext, normalize_repository_url,
};

pub const LATEST_SCHEMA_VERSION: i64 = 17;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, backup::Backup, params};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug)]
pub struct SqliteStore {
    connection: Connection,
    path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct PersistenceError(String);

impl PersistenceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    pub tenant_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceMember {
    pub actor_id: String,
    pub role: String,
    pub revoked_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceMembership {
    pub workspace: Workspace,
    pub role: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecificationRevision {
    pub specification_id: String,
    pub version: i64,
    pub content: String,
    pub content_hash: String,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceInvitation {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub email: String,
    pub role: String,
    pub token_hash: String,
    pub expires_at: String,
    pub accepted_at: Option<String>,
    pub revoked_at: Option<String>,
    pub created_by: String,
    pub created_at: String,
}

/// A browser session record. The bearer token is never persisted; callers
/// provide its SHA-256 digest and the store keeps only the actor binding and
/// lifecycle timestamps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSession {
    pub token_hash: String,
    pub actor_id: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyRecord {
    pub fingerprint: String,
    pub resource_type: String,
    pub resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerTask {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub controller_id: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub generation: i64,
    pub deduplication_key: String,
    pub status: String,
    pub attempts: i64,
    pub next_run_at: String,
    pub lease_until: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshotFile {
    pub snapshot_id: String,
    pub path: String,
    pub content_ref: String,
    pub content_hash: String,
    pub byte_length: i64,
    pub media_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretRecord {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub name: String,
    pub encrypted_value: String,
    pub key_version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationRecord {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub provider: String,
    pub display_name: String,
    pub secret_id: Option<String>,
    pub permissions: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySyncSchedule {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub ref_kind: String,
    pub ref_name: String,
    pub interval_seconds: i64,
    pub enabled: bool,
    pub next_run_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PersistenceError {}

impl From<std::io::Error> for PersistenceError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<rusqlite::Error> for PersistenceError {
    fn from(error: rusqlite::Error) -> Self {
        Self(error.to_string())
    }
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open(&path).map_err(PersistenceError::from)?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        let mut store = Self { connection, path: Some(path) };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, PersistenceError> {
        let connection = Connection::open_in_memory().map_err(PersistenceError::from)?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        let mut store = Self { connection, path: None };
        store.migrate()?;
        Ok(store)
    }

    pub fn database_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn migrate(&mut self) -> Result<(), PersistenceError> {
        let transaction = self.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS service_groups (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               parent_id TEXT REFERENCES service_groups(id), name TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0,
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
               group_id TEXT REFERENCES service_groups(id), name TEXT NOT NULL,
               slug TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL DEFAULT '',
               archived INTEGER NOT NULL DEFAULT 0,
               UNIQUE (tenant_id, workspace_id, slug)
             );
             CREATE TABLE IF NOT EXISTS code_sources (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               source_type TEXT NOT NULL, provider TEXT NOT NULL, display_name TEXT NOT NULL,
               canonical_key TEXT NOT NULL, root_locator TEXT NOT NULL, execution_scope TEXT NOT NULL,
               created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0,
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
               created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0,
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
               service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
               tag_id TEXT NOT NULL REFERENCES tags(id),
               PRIMARY KEY (tenant_id, workspace_id, service_id, tag_id)
             );
             CREATE TABLE IF NOT EXISTS specifications (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
               title TEXT NOT NULL, kind TEXT NOT NULL, content TEXT NOT NULL,
               version INTEGER NOT NULL DEFAULT 1, content_hash TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS specifications_scope ON specifications(tenant_id, workspace_id, service_id, updated_at);
             CREATE TABLE IF NOT EXISTS specification_revisions (
               id INTEGER PRIMARY KEY AUTOINCREMENT, specification_id TEXT NOT NULL REFERENCES specifications(id) ON DELETE CASCADE,
               tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL, version INTEGER NOT NULL,
               content TEXT NOT NULL, content_hash TEXT NOT NULL, created_by TEXT NOT NULL, created_at TEXT NOT NULL,
               UNIQUE (specification_id, version)
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
               id INTEGER PRIMARY KEY AUTOINCREMENT, tenant_id TEXT NOT NULL,
               workspace_id TEXT NOT NULL, actor_id TEXT NOT NULL, request_id TEXT NOT NULL,
               action TEXT NOT NULL, resource_type TEXT NOT NULL, resource_id TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS audit_events_scope ON audit_events(tenant_id, workspace_id, id);
             CREATE TABLE IF NOT EXISTS api_rate_limits (
               tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL, actor_id TEXT NOT NULL,
               window_start INTEGER NOT NULL, request_count INTEGER NOT NULL,
               PRIMARY KEY (tenant_id, workspace_id, actor_id)
             );
             CREATE TABLE IF NOT EXISTS api_idempotency (
               idempotency_key TEXT PRIMARY KEY,
               tenant_id TEXT NOT NULL,
               workspace_id TEXT NOT NULL,
               fingerprint TEXT NOT NULL,
               resource_type TEXT NOT NULL,
               resource_id TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS workspace_invitations (
               id TEXT PRIMARY KEY,
               tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               email TEXT NOT NULL, role TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
               expires_at TEXT NOT NULL, accepted_at TEXT, revoked_at TEXT, created_by TEXT NOT NULL, created_at TEXT NOT NULL
             );
             INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, datetime('now'));",
        )?;
        if !has_column(&transaction, "services", "updated_at")? {
            transaction.execute("ALTER TABLE services ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''", [])?;
        }
        if !has_column(&transaction, "services", "archived")? {
            transaction.execute("ALTER TABLE services ADD COLUMN archived INTEGER NOT NULL DEFAULT 0", [])?;
        }
        if !has_column(&transaction, "service_groups", "position")? {
            transaction.execute("ALTER TABLE service_groups ADD COLUMN position INTEGER NOT NULL DEFAULT 0", [])?;
        }
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (2, datetime('now'))", [])?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (3, datetime('now'))", [])?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (4, datetime('now'))", [])?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (5, datetime('now'))", [])?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS api_idempotency (
               idempotency_key TEXT PRIMARY KEY,
               tenant_id TEXT NOT NULL,
               workspace_id TEXT NOT NULL,
               fingerprint TEXT NOT NULL,
               resource_type TEXT NOT NULL,
               resource_id TEXT NOT NULL,
               created_at TEXT NOT NULL
             )",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (6, datetime('now'))", [])?;
        transaction.execute(
            "INSERT OR IGNORE INTO workspaces(tenant_id, id, name, slug, created_at)
             SELECT tenant_id, workspace_id,
                    CASE WHEN workspace_id = 'default' THEN '默认工作区' ELSE workspace_id END,
                    workspace_id, MIN(created_at)
             FROM (
               SELECT tenant_id, workspace_id, created_at FROM workspace_members
               UNION ALL SELECT tenant_id, workspace_id, datetime('now') FROM service_groups
               UNION ALL SELECT tenant_id, workspace_id, datetime('now') FROM tags
               UNION ALL SELECT tenant_id, workspace_id, datetime('now') FROM services
               UNION ALL SELECT tenant_id, workspace_id, datetime('now') FROM specifications
             ) existing_workspaces
             GROUP BY tenant_id, workspace_id",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (7, datetime('now'))", [])?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS workspace_invitations (
               id TEXT PRIMARY KEY,
               tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               email TEXT NOT NULL, role TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
               expires_at TEXT NOT NULL, accepted_at TEXT, revoked_at TEXT, created_by TEXT NOT NULL, created_at TEXT NOT NULL
             )",
            [],
        )?;
        if !has_column(&transaction, "workspace_invitations", "revoked_at")? {
            transaction.execute("ALTER TABLE workspace_invitations ADD COLUMN revoked_at TEXT", [])?;
        }
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (8, datetime('now'))", [])?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (9, datetime('now'))", [])?;
        transaction.execute(
            "CREATE INDEX IF NOT EXISTS idx_workspace_invitations_expires_at
             ON workspace_invitations(expires_at)",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (10, datetime('now'))", [])?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS web_sessions (
               token_hash TEXT PRIMARY KEY,
               actor_id TEXT NOT NULL,
               expires_at TEXT NOT NULL,
               revoked_at TEXT,
               created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_web_sessions_expires_at ON web_sessions(expires_at);",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (11, datetime('now'))", [])?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (12, datetime('now'))", [])?;
        transaction.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS specx_v13_legacy_repository_map(
               source_id TEXT PRIMARY KEY, repository_id TEXT NOT NULL
             );
             DELETE FROM specx_v13_legacy_repository_map;",
        )?;
        let legacy_git_sources = {
            let mut statement = transaction.prepare(
                "SELECT id,tenant_id,workspace_id,root_locator,created_at,updated_at,archived
                 FROM code_sources WHERE source_type='git' ORDER BY created_at,id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, bool>(6)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (source_id, tenant_id, workspace_id, root_locator, created_at, updated_at, archived) in legacy_git_sources {
            let Ok((canonical_url, provider, display_name)) = normalize_repository_url(&root_locator) else {
                continue;
            };
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT id FROM repositories WHERE tenant_id=?1 AND workspace_id=?2 AND canonical_url=?3",
                    params![tenant_id, workspace_id, canonical_url],
                    |row| row.get(0),
                )
                .optional()?;
            let repository_id = match existing {
                Some(id) => id,
                None => {
                    transaction.execute(
                        "INSERT OR IGNORE INTO repositories(id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        params![source_id, tenant_id, workspace_id, provider, display_name, canonical_url, created_at, updated_at, archived],
                    )?;
                    transaction.query_row(
                        "SELECT id FROM repositories WHERE tenant_id=?1 AND workspace_id=?2 AND canonical_url=?3",
                        params![tenant_id, workspace_id, canonical_url],
                        |row| row.get(0),
                    )?
                }
            };
            transaction.execute(
                "INSERT INTO specx_v13_legacy_repository_map(source_id,repository_id) VALUES(?1,?2)",
                params![source_id, repository_id],
            )?;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO service_repositories(id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at)
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
             )
             ON CONFLICT(tenant_id,workspace_id,service_id) DO NOTHING",
            [],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO service_ref_tracks(id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at)
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
             ORDER BY binding.created_at,binding.id",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (13, datetime('now'))", [])?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS specification_code_bindings (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               specification_id TEXT NOT NULL REFERENCES specifications(id) ON DELETE CASCADE,
               service_repository_id TEXT NOT NULL REFERENCES service_repositories(id) ON DELETE CASCADE,
               path TEXT NOT NULL DEFAULT '', ref_kind TEXT NOT NULL, ref_name TEXT NOT NULL,
               created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
               UNIQUE (tenant_id, workspace_id, specification_id)
             )",
            [],
        )?;
        transaction.execute(
            "CREATE INDEX IF NOT EXISTS specification_code_bindings_scope ON specification_code_bindings(tenant_id, workspace_id, specification_id)",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (14, datetime('now'))", [])?;
        transaction
            .execute("ALTER TABLE specification_code_bindings ADD COLUMN path TEXT NOT NULL DEFAULT ''", [])
            .or_else(|error| if error.to_string().contains("duplicate column name") { Ok(0) } else { Err(error) })?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (15, datetime('now'))", [])?;
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS repository_snapshots (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
               source_ref TEXT NOT NULL, resolved_commit TEXT NOT NULL, manifest_hash TEXT NOT NULL,
               content_root_ref TEXT, fetched_at TEXT NOT NULL, provider_version TEXT NOT NULL, created_at TEXT NOT NULL,
               UNIQUE (tenant_id, workspace_id, repository_id, source_ref, resolved_commit)
             );
             CREATE INDEX IF NOT EXISTS repository_snapshots_scope ON repository_snapshots(tenant_id, workspace_id, repository_id, fetched_at)",
            [],
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (16, datetime('now'))", [])?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS controller_tasks (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               controller_id TEXT NOT NULL, resource_kind TEXT NOT NULL, resource_id TEXT NOT NULL,
               generation INTEGER NOT NULL, deduplication_key TEXT NOT NULL,
               status TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, next_run_at TEXT NOT NULL,
               lease_until TEXT, last_error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
               UNIQUE (tenant_id, workspace_id, controller_id, deduplication_key)
             );
             CREATE INDEX IF NOT EXISTS controller_tasks_due ON controller_tasks(status, next_run_at, lease_until);
             CREATE TABLE IF NOT EXISTS repository_snapshot_files (
               snapshot_id TEXT NOT NULL REFERENCES repository_snapshots(id) ON DELETE CASCADE,
               path TEXT NOT NULL, content_ref TEXT NOT NULL, content_hash TEXT NOT NULL,
               byte_length INTEGER NOT NULL, media_type TEXT NOT NULL,
               PRIMARY KEY (snapshot_id, path)
             );
             CREATE TABLE IF NOT EXISTS secrets (
               id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
               name TEXT NOT NULL, encrypted_value TEXT NOT NULL, key_version INTEGER NOT NULL,
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
               ref_kind TEXT NOT NULL, ref_name TEXT NOT NULL, interval_seconds INTEGER NOT NULL,
               enabled INTEGER NOT NULL DEFAULT 1, next_run_at TEXT NOT NULL,
               created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
               UNIQUE (tenant_id, workspace_id, repository_id, ref_kind, ref_name)
             );
             CREATE INDEX IF NOT EXISTS repository_sync_schedules_due ON repository_sync_schedules(enabled, next_run_at);",
        )?;
        transaction
            .execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (17, datetime('now'))", [])?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(())
    }

    pub fn record_audit_event(&self, event: &AuditEvent) -> Result<(), PersistenceError> {
        insert_audit_event(&self.connection, event)
    }

    pub fn backup_to_path(&self, destination: impl AsRef<Path>) -> Result<(), PersistenceError> {
        let destination = destination.as_ref();
        if same_path(self.path.as_deref(), destination)? {
            return Err(PersistenceError("backup destination must differ from the live database".into()));
        }
        let file_name =
            destination.file_name().ok_or_else(|| PersistenceError("backup destination needs a file name".into()))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| PersistenceError(error.to_string()))?
            .as_nanos();
        let temporary =
            destination.with_file_name(format!("{}.tmp-{}-{stamp}", file_name.to_string_lossy(), std::process::id()));
        let result = (|| {
            let mut target = Connection::open(&temporary)?;
            {
                let backup = Backup::new(&self.connection, &mut target)?;
                backup.run_to_completion(64, Duration::from_millis(20), None)?;
            }
            ensure_integrity(&target)?;
            drop(target);
            std::fs::rename(&temporary, destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    /// Create a timestamped, integrity-checked snapshot and remove older
    /// snapshots with the same prefix. The destination directory is supplied
    /// by the process configuration, never by an API request.
    pub fn create_backup(
        &self,
        directory: impl AsRef<Path>,
        prefix: &str,
        retention: usize,
    ) -> Result<BackupArtifact, PersistenceError> {
        if prefix.is_empty()
            || prefix.len() > 80
            || !prefix.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(PersistenceError("backup prefix contains invalid characters".into()));
        }
        if retention == 0 {
            return Err(PersistenceError("backup retention must be at least one".into()));
        }
        let directory = directory.as_ref();
        std::fs::create_dir_all(directory)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| PersistenceError(error.to_string()))?
            .as_nanos();
        let destination = directory.join(format!("{prefix}-{stamp}.sqlite"));
        self.backup_to_path(&destination)?;
        let bytes = std::fs::metadata(&destination)?.len();
        let sha256 = file_sha256(&destination)?;

        let mut snapshots = std::fs::read_dir(directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path.extension().is_some_and(|extension| extension == "sqlite")
                    && path.file_stem().is_some_and(|stem| stem.to_string_lossy().starts_with(prefix))
            })
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|path| std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok());
        let excess = snapshots.len().saturating_sub(retention);
        for path in snapshots.into_iter().take(excess) {
            std::fs::remove_file(path)?;
        }

        Ok(BackupArtifact { path: destination, sha256, bytes })
    }

    pub fn create_workspace_backup(
        &self,
        context: &WorkspaceContext,
        directory: impl AsRef<Path>,
        prefix: &str,
        retention: usize,
    ) -> Result<BackupArtifact, PersistenceError> {
        let artifact = self.create_backup(directory, prefix, retention)?;
        let snapshot = Connection::open(&artifact.path)?;
        let transaction = snapshot.unchecked_transaction()?;
        transaction.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
        for table in [
            "service_tags",
            "service_ref_tracks",
            "service_repositories",
            "repository_snapshots",
            "specification_code_bindings",
            "specification_revisions",
            "specifications",
            "service_code_bindings",
            "code_sources",
            "repositories",
            "services",
            "service_groups",
            "tags",
            "workspace_members",
            "audit_events",
            "api_rate_limits",
            "api_idempotency",
            "workspace_invitations",
        ] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE tenant_id != ?1 OR workspace_id != ?2"),
                params![context.tenant_id, context.workspace_id],
            )?;
        }
        transaction.execute(
            "DELETE FROM workspaces WHERE tenant_id != ?1 OR id != ?2",
            params![context.tenant_id, context.workspace_id],
        )?;
        transaction.commit().map_err(PersistenceError::from)?;
        ensure_integrity(&snapshot)?;
        drop(snapshot);
        Ok(BackupArtifact {
            path: artifact.path.clone(),
            sha256: file_sha256(&artifact.path)?,
            bytes: std::fs::metadata(&artifact.path)?.len(),
        })
    }

    pub fn restore_workspace_from_path(
        &mut self,
        context: &WorkspaceContext,
        source: impl AsRef<Path>,
    ) -> Result<(), PersistenceError> {
        let source = source.as_ref();
        if same_path(self.path.as_deref(), source)? {
            return Err(PersistenceError("restore source must differ from the live database".into()));
        }
        let source_connection = Connection::open(source)?;
        ensure_integrity(&source_connection)?;
        let source_has_specification_code_bindings = has_table(&source_connection, "specification_code_bindings")?;
        let source_has_specification_binding_path = source_has_specification_code_bindings
            && has_column(&source_connection, "specification_code_bindings", "path")?;
        for table in [
            "service_groups",
            "tags",
            "services",
            "service_tags",
            "repositories",
            "repository_snapshots",
            "code_sources",
            "service_code_bindings",
            "service_repositories",
            "service_ref_tracks",
            "specification_code_bindings",
            "specifications",
            "specification_revisions",
            "workspace_members",
            "audit_events",
            "api_idempotency",
            "workspace_invitations",
        ] {
            if table == "specification_code_bindings" && !source_has_specification_code_bindings {
                continue;
            }
            let outside_scope: bool = source_connection.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE tenant_id != ?1 OR workspace_id != ?2)"),
                params![context.tenant_id, context.workspace_id],
                |row| row.get(0),
            )?;
            if outside_scope {
                return Err(PersistenceError(format!("workspace backup contains out-of-scope rows in {table}")));
            }
        }
        let outside_workspace_metadata: bool = source_connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE tenant_id != ?1 OR id != ?2)",
            params![context.tenant_id, context.workspace_id],
            |row| row.get(0),
        )?;
        if outside_workspace_metadata {
            return Err(PersistenceError::new("workspace backup contains out-of-scope workspace metadata"));
        }
        let source_has_invitation_revocation = has_column(&source_connection, "workspace_invitations", "revoked_at")?;
        drop(source_connection);
        self.connection.execute("ATTACH DATABASE ?1 AS workspace_backup", params![source.to_string_lossy()])?;
        let result = (|| {
            let transaction = self.connection.unchecked_transaction()?;
            transaction.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
            for table in [
                "service_tags",
                "service_ref_tracks",
                "service_repositories",
                "repository_snapshots",
                "service_code_bindings",
                "specification_revisions",
                "specifications",
                "services",
                "code_sources",
                "repositories",
                "service_groups",
                "tags",
                "workspace_members",
                "audit_events",
                "api_rate_limits",
            ] {
                transaction.execute(
                    &format!("DELETE FROM main.{table} WHERE tenant_id = ?1 AND workspace_id = ?2"),
                    params![context.tenant_id, context.workspace_id],
                )?;
            }
            transaction.execute(
                "DELETE FROM main.workspaces WHERE tenant_id = ?1 AND id = ?2",
                params![context.tenant_id, context.workspace_id],
            )?;
            let copy_tables = [
                ("service_groups", "id, tenant_id, workspace_id, parent_id, name, position"),
                ("tags", "id, tenant_id, workspace_id, name, color"),
                ("services", "id, tenant_id, workspace_id, group_id, name, slug, description, updated_at, archived"),
                (
                    "repositories",
                    "id, tenant_id, workspace_id, provider, display_name, canonical_url, created_at, updated_at, archived",
                ),
                (
                    "repository_snapshots",
                    "id, tenant_id, workspace_id, repository_id, source_ref, resolved_commit, manifest_hash, content_root_ref, fetched_at, provider_version, created_at",
                ),
                (
                    "code_sources",
                    "id, tenant_id, workspace_id, source_type, provider, display_name, canonical_key, root_locator, execution_scope, created_at, updated_at, archived",
                ),
                ("service_tags", "tenant_id, workspace_id, service_id, tag_id"),
                (
                    "service_code_bindings",
                    "id, tenant_id, workspace_id, service_id, source_id, role, entity_kind, path, revision_kind, revision, created_at, updated_at",
                ),
                (
                    "service_repositories",
                    "id, tenant_id, workspace_id, service_id, repository_id, created_at, updated_at",
                ),
                (
                    "service_ref_tracks",
                    "id, tenant_id, workspace_id, service_repository_id, ref_kind, ref_name, created_at, updated_at",
                ),
                (
                    "specification_code_bindings",
                    "id, tenant_id, workspace_id, specification_id, service_repository_id, path, ref_kind, ref_name, created_at, updated_at",
                ),
                (
                    "specifications",
                    "id, tenant_id, workspace_id, service_id, title, kind, content, version, content_hash, updated_at",
                ),
                (
                    "specification_revisions",
                    "specification_id, tenant_id, workspace_id, version, content, content_hash, created_by, created_at",
                ),
                ("workspace_members", "tenant_id, workspace_id, actor_id, role, revoked_at, created_at"),
                ("workspaces", "tenant_id, id, name, slug, created_at"),
                (
                    "audit_events",
                    "tenant_id, workspace_id, actor_id, request_id, action, resource_type, resource_id, created_at",
                ),
            ];
            for (table, columns) in copy_tables {
                if table == "specification_code_bindings" && !source_has_specification_code_bindings {
                    continue;
                }
                if table == "specification_code_bindings" && !source_has_specification_binding_path {
                    transaction.execute_batch("INSERT INTO main.specification_code_bindings(id,tenant_id,workspace_id,specification_id,service_repository_id,path,ref_kind,ref_name,created_at,updated_at) SELECT id,tenant_id,workspace_id,specification_id,service_repository_id,'',ref_kind,ref_name,created_at,updated_at FROM workspace_backup.specification_code_bindings;")?;
                } else {
                    transaction.execute_batch(&format!(
                        "INSERT INTO main.{table}({columns}) SELECT {columns} FROM workspace_backup.{table};"
                    ))?;
                }
            }
            let invitation_columns = if source_has_invitation_revocation {
                "id, tenant_id, workspace_id, email, role, token_hash, expires_at, accepted_at, revoked_at, created_by, created_at"
            } else {
                "id, tenant_id, workspace_id, email, role, token_hash, expires_at, accepted_at, NULL, created_by, created_at"
            };
            transaction.execute_batch(&format!(
                "INSERT INTO main.workspace_invitations(id, tenant_id, workspace_id, email, role, token_hash, expires_at, accepted_at, revoked_at, created_by, created_at)
                 SELECT {invitation_columns} FROM workspace_backup.workspace_invitations;"
            ))?;
            transaction.commit().map_err(PersistenceError::from)
        })();
        let detach = self.connection.execute_batch("DETACH DATABASE workspace_backup;");
        result?;
        detach.map_err(PersistenceError::from)?;
        ensure_integrity(&self.connection)
    }

    pub fn restore_from_path(&mut self, source: impl AsRef<Path>) -> Result<(), PersistenceError> {
        let source = source.as_ref();
        if same_path(self.path.as_deref(), source)? {
            return Err(PersistenceError("restore source must differ from the live database".into()));
        }
        let source_connection = Connection::open(source)?;
        ensure_integrity(&source_connection)?;
        {
            let backup = Backup::new(&source_connection, &mut self.connection)?;
            backup.run_to_completion(64, Duration::from_millis(20), None)?;
        }
        self.migrate()?;
        ensure_integrity(&self.connection)
    }

    pub fn audit_event_count(&self, context: &WorkspaceContext) -> Result<i64, PersistenceError> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE tenant_id = ?1 AND workspace_id = ?2",
                params![context.tenant_id, context.workspace_id],
                |row| row.get(0),
            )
            .map_err(PersistenceError::from)
    }

    pub fn create_web_session(&self, session: &WebSession) -> Result<(), PersistenceError> {
        self.connection.execute(
            "INSERT INTO web_sessions(token_hash,actor_id,expires_at,revoked_at,created_at)
             VALUES (?1,?2,?3,NULL,?4)
             ON CONFLICT(token_hash) DO UPDATE SET actor_id=excluded.actor_id, expires_at=excluded.expires_at,
               revoked_at=NULL, created_at=excluded.created_at",
            params![session.token_hash, session.actor_id, session.expires_at, session.created_at],
        )?;
        Ok(())
    }

    pub fn web_session_is_active(&self, token_hash: &str, now: &str) -> Result<bool, PersistenceError> {
        let active = self.connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM web_sessions
               WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2
             )",
            params![token_hash, now],
            |row| row.get::<_, bool>(0),
        )?;
        Ok(active)
    }

    pub fn web_session_actor_id(&self, token_hash: &str, now: &str) -> Result<Option<String>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT actor_id FROM web_sessions
                 WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2",
                params![token_hash, now],
                |row| row.get(0),
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn revoke_web_session(&self, token_hash: &str, revoked_at: &str) -> Result<bool, PersistenceError> {
        Ok(self.connection.execute(
            "UPDATE web_sessions SET revoked_at = ?1 WHERE token_hash = ?2 AND revoked_at IS NULL",
            params![revoked_at, token_hash],
        )? == 1)
    }

    pub fn revoke_other_web_sessions(&self, token_hash: &str, revoked_at: &str) -> Result<u64, PersistenceError> {
        Ok(self.connection.execute(
            "UPDATE web_sessions SET revoked_at = ?1 WHERE token_hash != ?2 AND revoked_at IS NULL",
            params![revoked_at, token_hash],
        )? as u64)
    }

    pub fn consume_rate_limit(
        &self,
        context: &WorkspaceContext,
        now: i64,
        window_seconds: i64,
        limit: u32,
    ) -> Result<bool, PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO api_rate_limits(tenant_id, workspace_id, actor_id, window_start, request_count)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(tenant_id, workspace_id, actor_id) DO UPDATE SET
               window_start = CASE
                 WHEN excluded.window_start >= api_rate_limits.window_start + ?5 THEN excluded.window_start
                 ELSE api_rate_limits.window_start END,
               request_count = CASE
                 WHEN excluded.window_start >= api_rate_limits.window_start + ?5 THEN 1
                 ELSE api_rate_limits.request_count + 1 END",
            params![context.tenant_id, context.workspace_id, context.actor_id, now, window_seconds],
        )?;
        let request_count: u32 = transaction.query_row(
            "SELECT request_count FROM api_rate_limits
             WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3",
            params![context.tenant_id, context.workspace_id, context.actor_id],
            |row| row.get(0),
        )?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(request_count <= limit)
    }

    pub fn authorize_workspace(&self, context: &WorkspaceContext) -> Result<bool, PersistenceError> {
        if self.workspace_member_role(context, &context.actor_id)?.is_some() {
            return Ok(true);
        }
        if context.workspace_id != "default" {
            return Ok(false);
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO workspaces(tenant_id, id, name, slug, created_at) VALUES (?1, ?2, '默认工作区', 'default', ?3)",
            params![context.tenant_id, context.workspace_id, Utc::now().to_rfc3339()],
        )?;
        self.connection.execute(
            "INSERT INTO workspace_members(tenant_id, workspace_id, actor_id, role, created_at)
             SELECT ?1, ?2, ?3, 'owner', ?4
             WHERE NOT EXISTS (
               SELECT 1 FROM workspace_members WHERE tenant_id = ?1 AND workspace_id = ?2
             )",
            params![context.tenant_id, context.workspace_id, context.actor_id, Utc::now().to_rfc3339()],
        )?;
        Ok(self.workspace_member_role(context, &context.actor_id)?.is_some())
    }

    pub fn workspaces_for_actor(
        &self,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<Vec<WorkspaceMembership>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT w.id, w.tenant_id, w.name, w.slug, w.created_at, m.role
             FROM workspaces w
             INNER JOIN workspace_members m
               ON m.tenant_id = w.tenant_id AND m.workspace_id = w.id
             WHERE w.tenant_id = ?1 AND m.actor_id = ?2 AND m.revoked_at IS NULL
             ORDER BY w.created_at, w.id",
        )?;
        let rows = statement.query_map(params![tenant_id, actor_id], |row| {
            Ok(WorkspaceMembership {
                workspace: Workspace {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    name: row.get(2)?,
                    slug: row.get(3)?,
                    created_at: row.get(4)?,
                },
                role: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PersistenceError::from)
    }

    pub fn create_workspace(
        &self,
        context: &WorkspaceContext,
        workspace: &Workspace,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace.tenant_id != context.tenant_id || workspace.id.is_empty() || workspace.name.trim().is_empty() {
            return Err(PersistenceError::new("invalid workspace metadata"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, context.actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            return Ok(false);
        }
        transaction.execute(
            "INSERT INTO workspaces(tenant_id, id, name, slug, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![workspace.tenant_id, workspace.id, workspace.name.trim(), workspace.slug, workspace.created_at],
        )?;
        transaction.execute(
            "INSERT INTO workspace_members(tenant_id, workspace_id, actor_id, role, created_at)
             VALUES (?1, ?2, ?3, 'owner', ?4)",
            params![workspace.tenant_id, workspace.id, context.actor_id, workspace.created_at],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn update_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        name: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace_id != context.workspace_id || name.trim().is_empty() {
            return Err(PersistenceError::new("invalid workspace update"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, context.actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            return Ok(false);
        }
        let changed = transaction.execute(
            "UPDATE workspaces SET name = ?1 WHERE tenant_id = ?2 AND id = ?3",
            params![name.trim(), context.tenant_id, workspace_id],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn delete_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if workspace_id != context.workspace_id || workspace_id == "default" {
            return Err(PersistenceError::new("workspace cannot be deleted"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, context.actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(role.as_deref(), Some("owner")) {
            return Ok(false);
        }
        transaction.execute(
            "DELETE FROM specification_revisions WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM specifications WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM service_tags WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM service_code_bindings WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM service_ref_tracks WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM specification_code_bindings WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM service_repositories WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM repositories WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM code_sources WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM services WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM tags WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM service_groups WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM workspace_invitations WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        transaction.execute(
            "DELETE FROM workspace_members WHERE tenant_id = ?1 AND workspace_id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        let changed = transaction.execute(
            "DELETE FROM workspaces WHERE tenant_id = ?1 AND id = ?2",
            params![context.tenant_id, workspace_id],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn delete_group(
        &self,
        context: &WorkspaceContext,
        group_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM service_groups WHERE tenant_id = ?1 AND workspace_id = ?2 AND id = ?3",
            params![context.tenant_id, context.workspace_id, group_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_tag(
        &self,
        context: &WorkspaceContext,
        tag_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM tags WHERE tenant_id = ?1 AND workspace_id = ?2 AND id = ?3",
            params![context.tenant_id, context.workspace_id, tag_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_service(
        &self,
        context: &WorkspaceContext,
        service_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM services WHERE tenant_id = ?1 AND workspace_id = ?2 AND id = ?3",
            params![context.tenant_id, context.workspace_id, service_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_code_source(
        &self,
        context: &WorkspaceContext,
        source_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM code_sources WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![context.tenant_id, context.workspace_id, source_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_code_binding(
        &self,
        context: &WorkspaceContext,
        binding_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM service_code_bindings WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![context.tenant_id, context.workspace_id, binding_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_service_repository(
        &self,
        context: &WorkspaceContext,
        service_repository_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM specification_code_bindings WHERE tenant_id=?1 AND workspace_id=?2 AND service_repository_id=?3",
            params![context.tenant_id, context.workspace_id, service_repository_id],
        )?;
        transaction.execute(
            "DELETE FROM service_repositories WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![context.tenant_id, context.workspace_id, service_repository_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_service_ref_track(
        &self,
        context: &WorkspaceContext,
        track_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM service_ref_tracks WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![context.tenant_id, context.workspace_id, track_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_specification_code_binding(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM specification_code_bindings WHERE tenant_id=?1 AND workspace_id=?2 AND specification_id=?3",
            params![context.tenant_id, context.workspace_id, specification_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn delete_specification(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM specification_revisions WHERE tenant_id = ?1 AND workspace_id = ?2 AND specification_id = ?3",
            params![context.tenant_id, context.workspace_id, specification_id],
        )?;
        transaction.execute(
            "DELETE FROM specifications WHERE tenant_id = ?1 AND workspace_id = ?2 AND id = ?3",
            params![context.tenant_id, context.workspace_id, specification_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
    ) -> Result<Option<String>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, actor_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn workspace_members(&self, context: &WorkspaceContext) -> Result<Vec<WorkspaceMember>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT actor_id, role, revoked_at, created_at FROM workspace_members
             WHERE tenant_id = ?1 AND workspace_id = ?2 ORDER BY actor_id",
        )?;
        let rows = statement.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(WorkspaceMember {
                actor_id: row.get(0)?,
                role: row.get(1)?,
                revoked_at: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PersistenceError::from)
    }

    pub fn create_workspace_invitation(
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
        let transaction = self.connection.unchecked_transaction()?;
        let role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, context.actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        transaction.execute(
            "INSERT INTO workspace_invitations(id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,NULL,NULL,?8,?9)",
            params![
                invitation.id,
                invitation.tenant_id,
                invitation.workspace_id,
                invitation.email.trim(),
                invitation.role,
                invitation.token_hash,
                invitation.expires_at,
                context.actor_id,
                invitation.created_at,
            ],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn workspace_invitations(
        &self,
        context: &WorkspaceContext,
    ) -> Result<Vec<WorkspaceInvitation>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at
             FROM workspace_invitations WHERE tenant_id = ?1 AND workspace_id = ?2 ORDER BY created_at DESC, id",
        )?;
        let rows = statement.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(WorkspaceInvitation {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                email: row.get(3)?,
                role: row.get(4)?,
                token_hash: row.get(5)?,
                expires_at: row.get(6)?,
                accepted_at: row.get(7)?,
                revoked_at: row.get(8)?,
                created_by: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PersistenceError::from)
    }

    /// Remove only expired invitations that were never accepted or revoked.
    /// Accepted and revoked rows remain available for audit/history queries.
    pub fn purge_expired_workspace_invitations(&self, now: &str) -> Result<u64, PersistenceError> {
        let changed = self.connection.execute(
            "DELETE FROM workspace_invitations
             WHERE expires_at <= ?1 AND accepted_at IS NULL AND revoked_at IS NULL",
            params![now],
        )?;
        Ok(changed as u64)
    }

    pub fn find_workspace_invitation(&self, token_hash: &str) -> Result<Option<WorkspaceInvitation>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at
                 FROM workspace_invitations WHERE token_hash = ?1",
                params![token_hash],
                |row| {
                    Ok(WorkspaceInvitation {
                        id: row.get(0)?,
                        tenant_id: row.get(1)?,
                        workspace_id: row.get(2)?,
                        email: row.get(3)?,
                        role: row.get(4)?,
                        token_hash: row.get(5)?,
                        expires_at: row.get(6)?,
                        accepted_at: row.get(7)?,
                        revoked_at: row.get(8)?,
                        created_by: row.get(9)?,
                        created_at: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn accept_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        token_hash: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<Option<WorkspaceMember>, PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        let invitation = transaction
            .query_row(
                "SELECT id,role,expires_at FROM workspace_invitations
                 WHERE token_hash = ?1 AND tenant_id = ?2 AND workspace_id = ?3 AND accepted_at IS NULL AND revoked_at IS NULL",
                params![token_hash, context.tenant_id, context.workspace_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()?;
        let Some((invitation_id, role, expires_at)) = invitation else {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(None);
        };
        if expires_at.as_str() <= now {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(None);
        }
        transaction.execute(
            "INSERT INTO workspace_members(tenant_id,workspace_id,actor_id,role,revoked_at,created_at)
             VALUES (?1,?2,?3,?4,NULL,?5)
             ON CONFLICT(tenant_id,workspace_id,actor_id) DO UPDATE SET role=excluded.role,revoked_at=NULL",
            params![context.tenant_id, context.workspace_id, context.actor_id, role, now],
        )?;
        transaction
            .execute("UPDATE workspace_invitations SET accepted_at = ?1 WHERE id = ?2", params![now, invitation_id])?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(Some(WorkspaceMember {
            actor_id: context.actor_id.clone(),
            role,
            revoked_at: None,
            created_at: now.to_owned(),
        }))
    }

    pub fn revoke_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        invitation_id: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        let role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, context.actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(role.as_deref(), Some("owner" | "admin")) {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        let changed = transaction.execute(
            "UPDATE workspace_invitations SET revoked_at = ?1
             WHERE id = ?2 AND tenant_id = ?3 AND workspace_id = ?4 AND accepted_at IS NULL AND revoked_at IS NULL",
            params![now, invitation_id, context.tenant_id, context.workspace_id],
        )?;
        if changed == 0 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn set_workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        role: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        if !matches!(role, "owner" | "admin" | "editor" | "viewer") {
            return Err(PersistenceError("invalid workspace member role".into()));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let current_role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, actor_id],
                |row| row.get(0),
            )
            .optional()?;
        if current_role.as_deref() == Some("owner") && role != "owner" {
            let owners: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND role = 'owner' AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id],
                |row| row.get(0),
            )?;
            if owners <= 1 {
                return Ok(false);
            }
        }
        let now = Utc::now().to_rfc3339();
        transaction.execute(
            "INSERT INTO workspace_members(tenant_id, workspace_id, actor_id, role, revoked_at, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)
             ON CONFLICT(tenant_id, workspace_id, actor_id) DO UPDATE SET role = excluded.role, revoked_at = NULL",
            params![context.tenant_id, context.workspace_id, actor_id, role, now],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn revoke_workspace_member(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        actor_id: &str,
    ) -> Result<bool, PersistenceError> {
        Ok(self.connection.execute(
            "UPDATE workspace_members SET revoked_at = ?1
                 WHERE tenant_id = ?2 AND workspace_id = ?3 AND actor_id = ?4 AND revoked_at IS NULL",
            params![Utc::now().to_rfc3339(), tenant_id, workspace_id, actor_id],
        )? == 1)
    }

    pub fn revoke_workspace_member_with_audit(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        let current_role: Option<String> = transaction
            .query_row(
                "SELECT role FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND actor_id = ?3 AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id, actor_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(current_role) = current_role else {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        };
        if current_role == "owner" {
            let owners: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM workspace_members
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND role = 'owner' AND revoked_at IS NULL",
                params![context.tenant_id, context.workspace_id],
                |row| row.get(0),
            )?;
            if owners <= 1 {
                return Ok(false);
            }
        }
        transaction.execute(
            "UPDATE workspace_members SET revoked_at = ?1
             WHERE tenant_id = ?2 AND workspace_id = ?3 AND actor_id = ?4 AND revoked_at IS NULL",
            params![Utc::now().to_rfc3339(), context.tenant_id, context.workspace_id, actor_id],
        )?;
        insert_audit_event(&transaction, audit)?;
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn persist_core(&self, core: &Core, context: &WorkspaceContext) -> Result<(), PersistenceError> {
        self.persist_core_with_audit(core, context, None)
    }

    pub fn persist_core_with_audit(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
    ) -> Result<(), PersistenceError> {
        self.persist_core_with_audit_and_idempotency(core, context, audit, None)
    }

    pub fn persist_core_with_audit_and_idempotency(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
        idempotency: Option<(&str, &str, &str, &str, &str, &str)>,
    ) -> Result<(), PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute_batch("PRAGMA defer_foreign_keys = ON;")?;

        for group in core
            .groups
            .values()
            .filter(|group| group.tenant_id == context.tenant_id && group.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO service_groups(id, tenant_id, workspace_id, parent_id, name, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET parent_id = excluded.parent_id, name = excluded.name, position = excluded.position
                 WHERE service_groups.tenant_id = excluded.tenant_id AND service_groups.workspace_id = excluded.workspace_id",
                params![group.id, context.tenant_id, group.workspace_id, group.parent_id, group.name, group.position],
            )?;
            ensure_upsert_scope(changed, "group")?;
        }
        for tag in core
            .tags
            .values()
            .filter(|tag| tag.tenant_id == context.tenant_id && tag.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO tags(id, tenant_id, workspace_id, name, color) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color
                 WHERE tags.tenant_id = excluded.tenant_id AND tags.workspace_id = excluded.workspace_id",
                params![tag.id, context.tenant_id, tag.workspace_id, tag.name, tag.color],
            )?;
            ensure_upsert_scope(changed, "tag")?;
        }
        for service in core
            .services
            .values()
            .filter(|service| service.tenant_id == context.tenant_id && service.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO services(id, tenant_id, workspace_id, group_id, name, slug, description, updated_at, archived) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET group_id = excluded.group_id, name = excluded.name, slug = excluded.slug, description = excluded.description, updated_at = excluded.updated_at, archived = excluded.archived
                 WHERE services.tenant_id = excluded.tenant_id AND services.workspace_id = excluded.workspace_id",
                params![service.id, service.tenant_id, service.workspace_id, service.group_id, service.name, service.slug, service.description, service.updated_at, service.archived],
            )?;
            ensure_upsert_scope(changed, "service")?;
            transaction.execute(
                "DELETE FROM service_tags WHERE tenant_id = ?1 AND workspace_id = ?2 AND service_id = ?3",
                params![context.tenant_id, context.workspace_id, service.id],
            )?;
            for tag_id in &service.tag_ids {
                transaction.execute(
                    "INSERT INTO service_tags(tenant_id, workspace_id, service_id, tag_id) VALUES (?1, ?2, ?3, ?4)",
                    params![context.tenant_id, context.workspace_id, service.id, tag_id],
                )?;
            }
        }
        for source in core
            .code_sources
            .values()
            .filter(|source| source.tenant_id == context.tenant_id && source.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO code_sources(id,tenant_id,workspace_id,source_type,provider,display_name,canonical_key,root_locator,execution_scope,created_at,updated_at,archived)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET source_type=excluded.source_type,provider=excluded.provider,display_name=excluded.display_name,
                   canonical_key=excluded.canonical_key,root_locator=excluded.root_locator,execution_scope=excluded.execution_scope,
                   created_at=excluded.created_at,updated_at=excluded.updated_at,archived=excluded.archived
                 WHERE code_sources.tenant_id=excluded.tenant_id AND code_sources.workspace_id=excluded.workspace_id",
                params![source.id, source.tenant_id, source.workspace_id, source_type_name(source.source_type), source.provider, source.display_name, source.canonical_key, source.root_locator, execution_scope_name(source.execution_scope), source.created_at, source.updated_at, source.archived],
            )?;
            ensure_upsert_scope(changed, "code source")?;
        }
        for binding in core
            .code_bindings
            .values()
            .filter(|binding| binding.tenant_id == context.tenant_id && binding.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO service_code_bindings(id,tenant_id,workspace_id,service_id,source_id,role,entity_kind,path,revision_kind,revision,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET service_id=excluded.service_id,source_id=excluded.source_id,role=excluded.role,
                   entity_kind=excluded.entity_kind,path=excluded.path,revision_kind=excluded.revision_kind,revision=excluded.revision,
                   created_at=excluded.created_at,updated_at=excluded.updated_at
                 WHERE service_code_bindings.tenant_id=excluded.tenant_id AND service_code_bindings.workspace_id=excluded.workspace_id",
                params![binding.id, binding.tenant_id, binding.workspace_id, binding.service_id, binding.source_id, binding_role_name(binding.role), entity_kind_name(binding.entity_kind), binding.path, revision_kind_name(binding.revision_kind), binding.revision, binding.created_at, binding.updated_at],
            )?;
            ensure_upsert_scope(changed, "code binding")?;
        }
        for repository in core.repositories.values().filter(|repository| {
            repository.tenant_id == context.tenant_id && repository.workspace_id == context.workspace_id
        }) {
            let changed = transaction.execute(
                "INSERT INTO repositories(id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET provider=excluded.provider,display_name=excluded.display_name,
                   canonical_url=excluded.canonical_url,updated_at=excluded.updated_at,archived=excluded.archived
                 WHERE repositories.tenant_id=excluded.tenant_id AND repositories.workspace_id=excluded.workspace_id",
                params![repository.id, repository.tenant_id, repository.workspace_id, repository.provider, repository.display_name, repository.canonical_url, repository.created_at, repository.updated_at, repository.archived],
            )?;
            ensure_upsert_scope(changed, "repository")?;
        }
        for snapshot in core
            .repository_snapshots
            .values()
            .filter(|snapshot| snapshot.tenant_id == context.tenant_id && snapshot.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
                   content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version
                 WHERE repository_snapshots.tenant_id=excluded.tenant_id AND repository_snapshots.workspace_id=excluded.workspace_id",
                params![snapshot.id, snapshot.tenant_id, snapshot.workspace_id, snapshot.repository_id, snapshot.source_ref, snapshot.resolved_commit, snapshot.manifest_hash, snapshot.content_root_ref, snapshot.fetched_at, snapshot.provider_version, snapshot.created_at],
            )?;
            ensure_upsert_scope(changed, "repository snapshot")?;
        }
        for relation in core
            .service_repositories
            .values()
            .filter(|relation| relation.tenant_id == context.tenant_id && relation.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO service_repositories(id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(id) DO UPDATE SET service_id=excluded.service_id,repository_id=excluded.repository_id,updated_at=excluded.updated_at
                 WHERE service_repositories.tenant_id=excluded.tenant_id AND service_repositories.workspace_id=excluded.workspace_id",
                params![relation.id, relation.tenant_id, relation.workspace_id, relation.service_id, relation.repository_id, relation.created_at, relation.updated_at],
            )?;
            ensure_upsert_scope(changed, "service repository")?;
        }
        for track in core
            .ref_tracks
            .values()
            .filter(|track| track.tenant_id == context.tenant_id && track.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO service_ref_tracks(id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(id) DO UPDATE SET service_repository_id=excluded.service_repository_id,ref_kind=excluded.ref_kind,
                   ref_name=excluded.ref_name,updated_at=excluded.updated_at
                 WHERE service_ref_tracks.tenant_id=excluded.tenant_id AND service_ref_tracks.workspace_id=excluded.workspace_id",
                params![track.id, track.tenant_id, track.workspace_id, track.service_repository_id, ref_kind_name(track.ref_kind), track.ref_name, track.created_at, track.updated_at],
            )?;
            ensure_upsert_scope(changed, "service ref track")?;
        }
        for binding in core
            .specification_code_bindings
            .values()
            .filter(|binding| binding.tenant_id == context.tenant_id && binding.workspace_id == context.workspace_id)
        {
            let changed = transaction.execute(
                "INSERT INTO specification_code_bindings(id,tenant_id,workspace_id,specification_id,service_repository_id,path,ref_kind,ref_name,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                 ON CONFLICT(id) DO UPDATE SET specification_id=excluded.specification_id,service_repository_id=excluded.service_repository_id,
                   path=excluded.path,ref_kind=excluded.ref_kind,ref_name=excluded.ref_name,updated_at=excluded.updated_at
                 WHERE specification_code_bindings.tenant_id=excluded.tenant_id AND specification_code_bindings.workspace_id=excluded.workspace_id",
                params![binding.id, binding.tenant_id, binding.workspace_id, binding.specification_id, binding.service_repository_id, binding.path, specification_ref_kind_name(binding.ref_kind), binding.ref_name, binding.created_at, binding.updated_at],
            )?;
            ensure_upsert_scope(changed, "specification code binding")?;
        }
        for specification in core.specifications.values().filter(|specification| {
            specification.tenant_id == context.tenant_id && specification.workspace_id == context.workspace_id
        }) {
            let hash = content_hash(&specification.content);
            let existing = transaction
                .query_row(
                    "SELECT version, content_hash, title, service_id, kind, updated_at FROM specifications WHERE id = ?1 AND tenant_id = ?2 AND workspace_id = ?3",
                    params![specification.id, context.tenant_id, context.workspace_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?)),
                )
                .optional()?;
            let now = Utc::now().to_rfc3339();
            let (version, content_changed, metadata_changed, updated_at) = match existing {
                Some((
                    current_version,
                    current_hash,
                    current_title,
                    current_service_id,
                    current_kind,
                    current_updated_at,
                )) => {
                    let content_changed = current_hash != hash;
                    let metadata_changed = current_title != specification.title
                        || current_service_id != specification.service_id
                        || current_kind != kind_name(specification.kind);
                    let updated_at = if content_changed || metadata_changed { now.clone() } else { current_updated_at };
                    (current_version + i64::from(content_changed), content_changed, metadata_changed, updated_at)
                }
                None => (
                    specification.version.max(1),
                    true,
                    true,
                    if specification.updated_at.is_empty() { now.clone() } else { specification.updated_at.clone() },
                ),
            };
            let updated_at = if content_changed || metadata_changed {
                if specification.updated_at.is_empty() { updated_at } else { specification.updated_at.clone() }
            } else {
                updated_at
            };
            let changed = transaction.execute(
                "INSERT INTO specifications(id, tenant_id, workspace_id, service_id, title, kind, content, version, content_hash, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET service_id = excluded.service_id, title = excluded.title, kind = excluded.kind, content = excluded.content, version = excluded.version, content_hash = excluded.content_hash, updated_at = excluded.updated_at
                 WHERE specifications.tenant_id = excluded.tenant_id AND specifications.workspace_id = excluded.workspace_id",
                params![specification.id, specification.tenant_id, specification.workspace_id, specification.service_id, specification.title, kind_name(specification.kind), specification.content, version, hash, updated_at],
            )?;
            ensure_upsert_scope(changed, "specification")?;
            if content_changed {
                transaction.execute(
                    "INSERT INTO specification_revisions(specification_id, tenant_id, workspace_id, version, content, content_hash, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![specification.id, specification.tenant_id, specification.workspace_id, version, specification.content, hash, context.actor_id, now],
                )?;
            }
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        if let Some((key, tenant_id, workspace_id, fingerprint, resource_type, resource_id)) = idempotency {
            transaction.execute(
                "INSERT INTO api_idempotency(idempotency_key, tenant_id, workspace_id, fingerprint, resource_type, resource_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![key, tenant_id, workspace_id, fingerprint, resource_type, resource_id, Utc::now().to_rfc3339()],
            )?;
        }
        transaction.commit().map_err(PersistenceError::from)
    }

    pub fn load_idempotency(&self, key: &str) -> Result<Option<IdempotencyRecord>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT fingerprint, resource_type, resource_id FROM api_idempotency WHERE idempotency_key = ?1",
                params![key],
                |row| {
                    Ok(IdempotencyRecord {
                        fingerprint: row.get(0)?,
                        resource_type: row.get(1)?,
                        resource_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn update_specification_if_current(
        &self,
        context: &WorkspaceContext,
        expected: &Specification,
        updated: &Specification,
    ) -> Result<bool, PersistenceError> {
        self.update_specification_if_current_with_audit(context, expected, updated, None)
    }

    pub fn update_specification_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &Specification,
        updated: &Specification,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        self.update_specification_if_current_with_audit_and_snapshot(context, expected, updated, audit, None)
    }

    pub fn update_specification_if_current_with_audit_and_snapshot(
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
            return Err(PersistenceError("specification update scope does not match".into()));
        }
        if let Some(snapshot) = snapshot
            && (snapshot.tenant_id != context.tenant_id || snapshot.workspace_id != context.workspace_id)
        {
            return Err(PersistenceError("repository snapshot scope does not match".into()));
        }

        let transaction = self.connection.unchecked_transaction()?;
        let hash = content_hash(&updated.content);
        let content_changed = expected.content != updated.content;
        let version = expected.version + i64::from(content_changed);
        let changed = transaction.execute(
            "UPDATE specifications
             SET title = ?1, content = ?2, version = ?3, content_hash = ?4, updated_at = ?5
             WHERE id = ?6 AND tenant_id = ?7 AND workspace_id = ?8 AND service_id = ?9
               AND title = ?10 AND kind = ?11 AND content = ?12 AND version = ?13
               AND content_hash = ?14 AND updated_at = ?15",
            params![
                updated.title,
                updated.content,
                version,
                hash,
                updated.updated_at,
                expected.id,
                context.tenant_id,
                context.workspace_id,
                expected.service_id,
                expected.title,
                kind_name(expected.kind),
                expected.content,
                expected.version,
                expected.content_hash,
                expected.updated_at,
            ],
        )?;
        if changed != 1 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }

        if content_changed {
            transaction.execute(
                "INSERT INTO specification_revisions(specification_id, tenant_id, workspace_id, version, content, content_hash, created_by, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    updated.id,
                    context.tenant_id,
                    context.workspace_id,
                    version,
                    updated.content,
                    hash,
                    context.actor_id,
                    updated.updated_at,
                ],
            )?;
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        if let Some(snapshot) = snapshot {
            transaction.execute(
                "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
                   content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version",
                params![snapshot.id, snapshot.tenant_id, snapshot.workspace_id, snapshot.repository_id, snapshot.source_ref, snapshot.resolved_commit, snapshot.manifest_hash, snapshot.content_root_ref, snapshot.fetched_at, snapshot.provider_version, snapshot.created_at],
            )?;
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn upsert_repository_snapshot(
        &self,
        context: &WorkspaceContext,
        snapshot: &RepositorySnapshot,
    ) -> Result<(), PersistenceError> {
        if snapshot.tenant_id != context.tenant_id || snapshot.workspace_id != context.workspace_id {
            return Err(PersistenceError("repository snapshot scope does not match".into()));
        }
        self.connection.execute(
            "INSERT INTO repository_snapshots(id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(tenant_id,workspace_id,repository_id,source_ref,resolved_commit) DO UPDATE SET manifest_hash=excluded.manifest_hash,
               content_root_ref=excluded.content_root_ref,fetched_at=excluded.fetched_at,provider_version=excluded.provider_version",
            params![snapshot.id, snapshot.tenant_id, snapshot.workspace_id, snapshot.repository_id, snapshot.source_ref, snapshot.resolved_commit, snapshot.manifest_hash, snapshot.content_root_ref, snapshot.fetched_at, snapshot.provider_version, snapshot.created_at],
        )?;
        Ok(())
    }

    pub fn upsert_repository_snapshot_file(&self, file: &RepositorySnapshotFile) -> Result<(), PersistenceError> {
        self.connection.execute(
            "INSERT INTO repository_snapshot_files(snapshot_id,path,content_ref,content_hash,byte_length,media_type)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(snapshot_id,path) DO UPDATE SET content_ref=excluded.content_ref,content_hash=excluded.content_hash,
               byte_length=excluded.byte_length,media_type=excluded.media_type",
            params![file.snapshot_id, file.path, file.content_ref, file.content_hash, file.byte_length, file.media_type],
        )?;
        Ok(())
    }

    pub fn repository_snapshot_files(
        &self,
        snapshot_id: &str,
    ) -> Result<Vec<RepositorySnapshotFile>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT snapshot_id,path,content_ref,content_hash,byte_length,media_type
             FROM repository_snapshot_files WHERE snapshot_id=?1 ORDER BY path",
        )?;
        let files = statement
            .query_map(params![snapshot_id], |row| {
                Ok(RepositorySnapshotFile {
                    snapshot_id: row.get(0)?,
                    path: row.get(1)?,
                    content_ref: row.get(2)?,
                    content_hash: row.get(3)?,
                    byte_length: row.get(4)?,
                    media_type: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(files)
    }

    pub fn enqueue_controller_task(&self, task: &ControllerTask) -> Result<bool, PersistenceError> {
        let changed = self.connection.execute(
            "INSERT INTO controller_tasks(id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'pending',0,?9,NULL,NULL,?10,?10)
             ON CONFLICT(tenant_id,workspace_id,controller_id,deduplication_key) DO UPDATE SET status='pending',next_run_at=excluded.next_run_at,lease_until=NULL,last_error=NULL,updated_at=excluded.updated_at
             WHERE controller_tasks.status IN ('succeeded','failed')",
            params![task.id, task.tenant_id, task.workspace_id, task.controller_id, task.resource_kind, task.resource_id, task.generation, task.deduplication_key, task.next_run_at, task.created_at],
        )?;
        Ok(changed == 1)
    }

    pub fn claim_due_controller_tasks(
        &self,
        now: &str,
        lease_until: &str,
        limit: usize,
    ) -> Result<Vec<ControllerTask>, PersistenceError> {
        let transaction = self.connection.unchecked_transaction()?;
        let mut statement = transaction.prepare(
            "SELECT id FROM controller_tasks
             WHERE (status='pending' AND next_run_at<=?1) OR (status='running' AND lease_until<=?1)
             ORDER BY next_run_at,id LIMIT ?2",
        )?;
        let ids = statement
            .query_map(params![now, limit as i64], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for id in &ids {
            transaction.execute(
                "UPDATE controller_tasks SET status='running',attempts=attempts+1,lease_until=?1,updated_at=?2 WHERE id=?3",
                params![lease_until, now, id],
            )?;
        }
        let mut tasks = Vec::with_capacity(ids.len());
        for id in ids {
            tasks.push(transaction.query_row(
                "SELECT id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at FROM controller_tasks WHERE id=?1",
                params![id],
                controller_task_from_row,
            )?);
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(tasks)
    }

    pub fn controller_task(&self, task_id: &str) -> Result<Option<ControllerTask>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT id,tenant_id,workspace_id,controller_id,resource_kind,resource_id,generation,deduplication_key,status,attempts,next_run_at,lease_until,last_error,created_at,updated_at FROM controller_tasks WHERE id=?1",
                params![task_id],
                controller_task_from_row,
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn complete_controller_task(&self, task_id: &str, now: &str) -> Result<bool, PersistenceError> {
        Ok(self.connection.execute(
            "UPDATE controller_tasks SET status='succeeded',lease_until=NULL,last_error=NULL,updated_at=?1 WHERE id=?2 AND status='running'",
            params![now, task_id],
        )? == 1)
    }

    pub fn fail_controller_task(
        &self,
        task_id: &str,
        retry: bool,
        error: &str,
        next_run_at: &str,
        now: &str,
    ) -> Result<bool, PersistenceError> {
        let status = if retry { "pending" } else { "failed" };
        Ok(self.connection.execute(
            "UPDATE controller_tasks SET status=?1,lease_until=NULL,last_error=?2,next_run_at=?3,updated_at=?4 WHERE id=?5 AND status='running'",
            params![status, error, next_run_at, now, task_id],
        )? == 1)
    }

    pub fn upsert_repository_sync_schedule(&self, schedule: &RepositorySyncSchedule) -> Result<(), PersistenceError> {
        self.connection.execute(
            "INSERT INTO repository_sync_schedules(id,tenant_id,workspace_id,repository_id,ref_kind,ref_name,interval_seconds,enabled,next_run_at,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)
             ON CONFLICT(tenant_id,workspace_id,repository_id,ref_kind,ref_name) DO UPDATE SET interval_seconds=excluded.interval_seconds,enabled=excluded.enabled,next_run_at=excluded.next_run_at,updated_at=excluded.updated_at",
            params![schedule.id, schedule.tenant_id, schedule.workspace_id, schedule.repository_id, schedule.ref_kind, schedule.ref_name, schedule.interval_seconds, schedule.enabled, schedule.next_run_at, schedule.created_at],
        )?;
        Ok(())
    }

    pub fn due_repository_sync_schedules(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<RepositorySyncSchedule>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT id,tenant_id,workspace_id,repository_id,ref_kind,ref_name,interval_seconds,enabled,next_run_at,created_at,updated_at
             FROM repository_sync_schedules WHERE enabled=1 AND next_run_at<=?1 ORDER BY next_run_at,id LIMIT ?2",
        )?;
        let schedules = statement
            .query_map(params![now, limit as i64], repository_schedule_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(schedules)
    }

    pub fn advance_repository_sync_schedule(
        &self,
        id: &str,
        next_run_at: &str,
        now: &str,
    ) -> Result<bool, PersistenceError> {
        Ok(self.connection.execute(
            "UPDATE repository_sync_schedules SET next_run_at=?1,updated_at=?2 WHERE id=?3",
            params![next_run_at, now, id],
        )? == 1)
    }

    pub fn upsert_secret(&self, secret: &SecretRecord) -> Result<(), PersistenceError> {
        self.connection.execute(
            "INSERT INTO secrets(id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(tenant_id,workspace_id,name) DO UPDATE SET encrypted_value=excluded.encrypted_value,key_version=excluded.key_version,updated_at=excluded.updated_at",
            params![secret.id,secret.tenant_id,secret.workspace_id,secret.name,secret.encrypted_value,secret.key_version,secret.created_at,secret.updated_at],
        )?;
        Ok(())
    }

    pub fn secret_by_id(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<SecretRecord>, PersistenceError> {
        self.connection.query_row(
            "SELECT id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at FROM secrets WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![tenant_id,workspace_id,id],
            |row| Ok(SecretRecord { id:row.get(0)?,tenant_id:row.get(1)?,workspace_id:row.get(2)?,name:row.get(3)?,encrypted_value:row.get(4)?,key_version:row.get(5)?,created_at:row.get(6)?,updated_at:row.get(7)? }),
        ).optional().map_err(PersistenceError::from)
    }

    pub fn list_secrets(&self, tenant_id: &str, workspace_id: &str) -> Result<Vec<SecretRecord>, PersistenceError> {
        let mut statement = self.connection.prepare("SELECT id,tenant_id,workspace_id,name,encrypted_value,key_version,created_at,updated_at FROM secrets WHERE tenant_id=?1 AND workspace_id=?2 ORDER BY name,id")?;
        let result = statement
            .query_map(params![tenant_id, workspace_id], |row| {
                Ok(SecretRecord {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    workspace_id: row.get(2)?,
                    name: row.get(3)?,
                    encrypted_value: row.get(4)?,
                    key_version: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    }

    pub fn delete_secret(&self, tenant_id: &str, workspace_id: &str, id: &str) -> Result<bool, PersistenceError> {
        Ok(self.connection.execute(
            "DELETE FROM secrets WHERE tenant_id=?1 AND workspace_id=?2 AND id=?3",
            params![tenant_id, workspace_id, id],
        )? == 1)
    }

    pub fn upsert_integration(&self, integration: &IntegrationRecord) -> Result<(), PersistenceError> {
        self.connection.execute(
            "INSERT INTO integrations(id,tenant_id,workspace_id,provider,display_name,secret_id,permissions,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(tenant_id,workspace_id,provider,display_name) DO UPDATE SET secret_id=excluded.secret_id,permissions=excluded.permissions,updated_at=excluded.updated_at",
            params![integration.id,integration.tenant_id,integration.workspace_id,integration.provider,integration.display_name,integration.secret_id,integration.permissions,integration.created_at,integration.updated_at],
        )?;
        Ok(())
    }

    pub fn list_integrations(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<IntegrationRecord>, PersistenceError> {
        let mut statement = self.connection.prepare("SELECT id,tenant_id,workspace_id,provider,display_name,secret_id,permissions,created_at,updated_at FROM integrations WHERE tenant_id=?1 AND workspace_id=?2 ORDER BY display_name,id")?;
        let result = statement
            .query_map(params![tenant_id, workspace_id], |row| {
                Ok(IntegrationRecord {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    workspace_id: row.get(2)?,
                    provider: row.get(3)?,
                    display_name: row.get(4)?,
                    secret_id: row.get(5)?,
                    permissions: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    }

    pub fn update_specification_code_binding_if_current_with_audit(
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
            return Err(PersistenceError("specification code binding update scope does not match".into()));
        }

        let transaction = self.connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE specification_code_bindings
             SET service_repository_id=?1,ref_kind=?2,ref_name=?3,updated_at=?4
             WHERE id=?5 AND tenant_id=?6 AND workspace_id=?7 AND specification_id=?8
               AND service_repository_id=?9 AND ref_kind=?10 AND ref_name=?11 AND updated_at=?12",
            params![
                updated.service_repository_id,
                specification_ref_kind_name(updated.ref_kind),
                updated.ref_name,
                updated.updated_at,
                expected.id,
                context.tenant_id,
                context.workspace_id,
                expected.specification_id,
                expected.service_repository_id,
                specification_ref_kind_name(expected.ref_kind),
                expected.ref_name,
                expected.updated_at,
            ],
        )?;
        if changed != 1 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn update_service_if_current_with_audit(
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
            return Err(PersistenceError("service update scope does not match".into()));
        }

        let transaction = self.connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE services SET group_id=?1,name=?2,slug=?3,description=?4,updated_at=?5,archived=?6
             WHERE id=?7 AND tenant_id=?8 AND workspace_id=?9 AND group_id IS ?10 AND name=?11
               AND slug=?12 AND description=?13 AND updated_at=?14 AND archived=?15",
            params![
                updated.group_id,
                updated.name,
                updated.slug,
                updated.description,
                updated.updated_at,
                updated.archived,
                expected.id,
                context.tenant_id,
                context.workspace_id,
                expected.group_id,
                expected.name,
                expected.slug,
                expected.description,
                expected.updated_at,
                expected.archived,
            ],
        )?;
        if changed != 1 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }

        transaction.execute(
            "DELETE FROM service_tags WHERE tenant_id=?1 AND workspace_id=?2 AND service_id=?3",
            params![context.tenant_id, context.workspace_id, updated.id],
        )?;
        for tag_id in &updated.tag_ids {
            transaction.execute(
                "INSERT INTO service_tags(tenant_id,workspace_id,service_id,tag_id) VALUES(?1,?2,?3,?4)",
                params![context.tenant_id, context.workspace_id, updated.id, tag_id],
            )?;
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn update_code_source_if_current_with_audit(
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
            return Err(PersistenceError("code source update scope does not match".into()));
        }

        let transaction = self.connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE code_sources SET source_type=?1,provider=?2,display_name=?3,canonical_key=?4,
               root_locator=?5,execution_scope=?6,updated_at=?7,archived=?8
             WHERE id=?9 AND tenant_id=?10 AND workspace_id=?11 AND source_type=?12 AND provider=?13
               AND display_name=?14 AND canonical_key=?15 AND root_locator=?16 AND execution_scope=?17
               AND updated_at=?18 AND archived=?19",
            params![
                source_type_name(updated.source_type),
                updated.provider,
                updated.display_name,
                updated.canonical_key,
                updated.root_locator,
                execution_scope_name(updated.execution_scope),
                updated.updated_at,
                updated.archived,
                expected.id,
                context.tenant_id,
                context.workspace_id,
                source_type_name(expected.source_type),
                expected.provider,
                expected.display_name,
                expected.canonical_key,
                expected.root_locator,
                execution_scope_name(expected.execution_scope),
                expected.updated_at,
                expected.archived,
            ],
        )?;
        if changed != 1 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn update_code_binding_if_current_with_audit(
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
            return Err(PersistenceError("code binding update scope does not match".into()));
        }

        let transaction = self.connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE service_code_bindings SET role=?1,entity_kind=?2,path=?3,revision_kind=?4,
               revision=?5,updated_at=?6
             WHERE id=?7 AND tenant_id=?8 AND workspace_id=?9 AND service_id=?10 AND source_id=?11
               AND role=?12 AND entity_kind=?13 AND path=?14 AND revision_kind=?15 AND revision IS ?16
               AND updated_at=?17",
            params![
                binding_role_name(updated.role),
                entity_kind_name(updated.entity_kind),
                updated.path,
                revision_kind_name(updated.revision_kind),
                updated.revision,
                updated.updated_at,
                expected.id,
                context.tenant_id,
                context.workspace_id,
                expected.service_id,
                expected.source_id,
                binding_role_name(expected.role),
                entity_kind_name(expected.entity_kind),
                expected.path,
                revision_kind_name(expected.revision_kind),
                expected.revision,
                expected.updated_at,
            ],
        )?;
        if changed != 1 {
            transaction.commit().map_err(PersistenceError::from)?;
            return Ok(false);
        }
        if let Some(event) = audit {
            insert_audit_event(&transaction, event)?;
        }
        transaction.commit().map_err(PersistenceError::from)?;
        Ok(true)
    }

    pub fn load_core(&self, context: &WorkspaceContext) -> Result<Core, PersistenceError> {
        let mut core = Core::default();
        let mut groups = self.connection.prepare("SELECT id, tenant_id, workspace_id, parent_id, name, position FROM service_groups WHERE tenant_id = ?1 AND workspace_id = ?2")?;
        for row in groups.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(ServiceGroup {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                parent_id: row.get(3)?,
                name: row.get(4)?,
                position: row.get(5)?,
            })
        })? {
            let group = row?;
            core.groups.insert(group.id.clone(), group);
        }
        let mut tags = self.connection.prepare(
            "SELECT id, tenant_id, workspace_id, name, color FROM tags WHERE tenant_id = ?1 AND workspace_id = ?2",
        )?;
        for row in tags.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                name: row.get(3)?,
                color: row.get(4)?,
            })
        })? {
            let tag = row?;
            core.tags.insert(tag.id.clone(), tag);
        }
        let mut services = self.connection.prepare("SELECT id, tenant_id, workspace_id, group_id, name, slug, description, updated_at, archived FROM services WHERE tenant_id = ?1 AND workspace_id = ?2")?;
        for row in services.query_map(params![context.tenant_id, context.workspace_id], |row| {
            let id: Id = row.get(0)?;
            let mut tags = self.connection.prepare("SELECT tag_id FROM service_tags WHERE tenant_id = ?1 AND workspace_id = ?2 AND service_id = ?3 ORDER BY tag_id")?;
            let tag_ids = tags.query_map(params![context.tenant_id, context.workspace_id, id], |tag| tag.get(0))?.collect::<Result<Vec<String>, _>>()?;
            Ok(Service { id, tenant_id: row.get(1)?, workspace_id: row.get(2)?, group_id: row.get(3)?, name: row.get(4)?, slug: row.get(5)?, description: row.get(6)?, tag_ids, updated_at: row.get(7)?, archived: row.get(8)? })
        })? {
            let service = row?;
            core.services.insert(service.id.clone(), service);
        }
        let mut specifications = self.connection.prepare("SELECT id, tenant_id, workspace_id, service_id, title, kind, content, version, content_hash, updated_at FROM specifications WHERE tenant_id = ?1 AND workspace_id = ?2")?;
        for row in specifications.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(Specification {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                service_id: row.get(3)?,
                title: row.get(4)?,
                kind: parse_kind(&row.get::<_, String>(5)?)?,
                content: row.get(6)?,
                version: row.get(7)?,
                content_hash: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })? {
            let specification = row?;
            core.specifications.insert(specification.id.clone(), specification);
        }
        let mut sources = self.connection.prepare("SELECT id,tenant_id,workspace_id,source_type,provider,display_name,canonical_key,root_locator,execution_scope,created_at,updated_at,archived FROM code_sources WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in sources.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(CodeSource {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                source_type: parse_source_type(&row.get::<_, String>(3)?)?,
                provider: row.get(4)?,
                display_name: row.get(5)?,
                canonical_key: row.get(6)?,
                root_locator: row.get(7)?,
                execution_scope: parse_execution_scope(&row.get::<_, String>(8)?)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                archived: row.get(11)?,
            })
        })? {
            let source = row?;
            core.code_sources.insert(source.id.clone(), source);
        }
        let mut bindings = self.connection.prepare("SELECT id,tenant_id,workspace_id,service_id,source_id,role,entity_kind,path,revision_kind,revision,created_at,updated_at FROM service_code_bindings WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in bindings.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(ServiceCodeBinding {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                service_id: row.get(3)?,
                source_id: row.get(4)?,
                role: parse_binding_role(&row.get::<_, String>(5)?)?,
                entity_kind: parse_entity_kind(&row.get::<_, String>(6)?)?,
                path: row.get(7)?,
                revision_kind: parse_revision_kind(&row.get::<_, String>(8)?)?,
                revision: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })? {
            let binding = row?;
            core.code_bindings.insert(binding.id.clone(), binding);
        }
        let mut repositories = self.connection.prepare("SELECT id,tenant_id,workspace_id,provider,display_name,canonical_url,created_at,updated_at,archived FROM repositories WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in repositories.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(Repository {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                provider: row.get(3)?,
                display_name: row.get(4)?,
                canonical_url: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                archived: row.get(8)?,
            })
        })? {
            let repository = row?;
            core.repositories.insert(repository.id.clone(), repository);
        }
        let mut repository_snapshots = self.connection.prepare("SELECT id,tenant_id,workspace_id,repository_id,source_ref,resolved_commit,manifest_hash,content_root_ref,fetched_at,provider_version,created_at FROM repository_snapshots WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in repository_snapshots.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(RepositorySnapshot {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                repository_id: row.get(3)?,
                source_ref: row.get(4)?,
                resolved_commit: row.get(5)?,
                manifest_hash: row.get(6)?,
                content_root_ref: row.get(7)?,
                fetched_at: row.get(8)?,
                provider_version: row.get(9)?,
                created_at: row.get(10)?,
            })
        })? {
            let snapshot = row?;
            core.repository_snapshots.insert(snapshot.id.clone(), snapshot);
        }
        let mut relations = self.connection.prepare("SELECT id,tenant_id,workspace_id,service_id,repository_id,created_at,updated_at FROM service_repositories WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in relations.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(ServiceRepository {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                service_id: row.get(3)?,
                repository_id: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })? {
            let relation = row?;
            core.service_repositories.insert(relation.id.clone(), relation);
        }
        let mut tracks = self.connection.prepare("SELECT id,tenant_id,workspace_id,service_repository_id,ref_kind,ref_name,created_at,updated_at FROM service_ref_tracks WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in tracks.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(ServiceRefTrack {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                service_repository_id: row.get(3)?,
                ref_kind: parse_ref_kind(&row.get::<_, String>(4)?)?,
                ref_name: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })? {
            let track = row?;
            core.ref_tracks.insert(track.id.clone(), track);
        }
        let mut specification_bindings = self.connection.prepare("SELECT id,tenant_id,workspace_id,specification_id,service_repository_id,path,ref_kind,ref_name,created_at,updated_at FROM specification_code_bindings WHERE tenant_id=?1 AND workspace_id=?2")?;
        for row in specification_bindings.query_map(params![context.tenant_id, context.workspace_id], |row| {
            Ok(SpecificationCodeBinding {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                workspace_id: row.get(2)?,
                specification_id: row.get(3)?,
                service_repository_id: row.get(4)?,
                path: row.get(5)?,
                ref_kind: parse_specification_ref_kind(&row.get::<_, String>(6)?)?,
                ref_name: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })? {
            let binding = row?;
            core.specification_code_bindings.insert(binding.id.clone(), binding);
        }
        Ok(core)
    }

    pub fn specification_revision_count(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Result<i64, PersistenceError> {
        self.connection.query_row("SELECT COUNT(*) FROM specification_revisions WHERE tenant_id = ?1 AND workspace_id = ?2 AND specification_id = ?3", params![context.tenant_id, context.workspace_id, specification_id], |row| row.get(0)).map_err(PersistenceError::from)
    }

    pub fn specification_revisions(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SpecificationRevision>, PersistenceError> {
        let mut statement = self.connection.prepare(
            "SELECT specification_id, version, content, content_hash, created_by, created_at
             FROM specification_revisions
             WHERE tenant_id = ?1 AND workspace_id = ?2 AND specification_id = ?3
             ORDER BY version DESC LIMIT ?4 OFFSET ?5",
        )?;
        statement
            .query_map(
                params![context.tenant_id, context.workspace_id, specification_id, limit as i64, offset as i64],
                |row| {
                    Ok(SpecificationRevision {
                        specification_id: row.get(0)?,
                        version: row.get(1)?,
                        content: row.get(2)?,
                        content_hash: row.get(3)?,
                        created_by: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(PersistenceError::from)
    }

    pub fn specification_revision(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        version: i64,
    ) -> Result<Option<SpecificationRevision>, PersistenceError> {
        self.connection
            .query_row(
                "SELECT specification_id, version, content, content_hash, created_by, created_at
                 FROM specification_revisions
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND specification_id = ?3 AND version = ?4",
                params![context.tenant_id, context.workspace_id, specification_id, version],
                |row| {
                    Ok(SpecificationRevision {
                        specification_id: row.get(0)?,
                        version: row.get(1)?,
                        content: row.get(2)?,
                        content_hash: row.get(3)?,
                        created_by: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn has_migration(&self, version: i64) -> Result<bool, PersistenceError> {
        self.connection
            .query_row("SELECT 1 FROM schema_migrations WHERE version = ?1", params![version], |_| Ok(true))
            .optional()
            .map(|value| value.is_some())
            .map_err(PersistenceError::from)
    }

    pub fn ready(&self) -> Result<bool, PersistenceError> {
        self.has_migration(LATEST_SCHEMA_VERSION)
    }
}

fn has_column(connection: &Connection, table: &str, column: &str) -> Result<bool, PersistenceError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_table(connection: &Connection, table: &str) -> Result<bool, PersistenceError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |row| row.get(0),
        )
        .map_err(PersistenceError::from)
}

fn ensure_integrity(connection: &Connection) -> Result<(), PersistenceError> {
    let status: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if status != "ok" {
        return Err(PersistenceError(format!("SQLite integrity check failed: {status}")));
    }
    let migrations_table: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    let latest_migration: i64 = if migrations_table {
        connection.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |row| row.get(0))?
    } else {
        0
    };
    if latest_migration < 1 {
        return Err(PersistenceError("SQLite snapshot does not contain a supported specX schema".into()));
    }
    Ok(())
}

fn same_path(current: Option<&Path>, candidate: &Path) -> Result<bool, PersistenceError> {
    let Some(current) = current else { return Ok(false) };
    if !current.exists() || !candidate.exists() {
        return Ok(current == candidate);
    }
    Ok(std::fs::canonicalize(current)? == std::fs::canonicalize(candidate)?)
}

fn insert_audit_event(connection: &Connection, event: &AuditEvent) -> Result<(), PersistenceError> {
    connection.execute(
        "INSERT INTO audit_events(tenant_id, workspace_id, actor_id, request_id, action, resource_type, resource_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event.tenant_id,
            event.workspace_id,
            event.actor_id,
            event.request_id,
            event.action,
            event.resource_type,
            event.resource_id,
            event.created_at,
        ],
    )?;
    Ok(())
}

fn ensure_upsert_scope(changed: usize, entity: &str) -> Result<(), PersistenceError> {
    if changed == 1 { Ok(()) } else { Err(PersistenceError(format!("{entity} id belongs to a different scope"))) }
}

fn kind_name(kind: SpecificationKind) -> &'static str {
    match kind {
        SpecificationKind::Markdown => "markdown",
        SpecificationKind::OpenApi => "openapi",
    }
}

fn source_type_name(value: CodeSourceType) -> &'static str {
    match value {
        CodeSourceType::Git => "git",
        CodeSourceType::LocalFilesystem => "local_filesystem",
    }
}
fn parse_source_type(value: &str) -> Result<CodeSourceType, rusqlite::Error> {
    match value {
        "git" => Ok(CodeSourceType::Git),
        "local_filesystem" => Ok(CodeSourceType::LocalFilesystem),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn execution_scope_name(value: CodeExecutionScope) -> &'static str {
    match value {
        CodeExecutionScope::Server => "server",
        CodeExecutionScope::DesktopHost => "desktop_host",
    }
}
fn parse_execution_scope(value: &str) -> Result<CodeExecutionScope, rusqlite::Error> {
    match value {
        "server" => Ok(CodeExecutionScope::Server),
        "desktop_host" => Ok(CodeExecutionScope::DesktopHost),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn binding_role_name(value: CodeBindingRole) -> &'static str {
    match value {
        CodeBindingRole::Primary => "primary",
        CodeBindingRole::Additional => "additional",
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
fn parse_ref_kind(value: &str) -> Result<ServiceRefKind, rusqlite::Error> {
    match value {
        "default_branch" => Ok(ServiceRefKind::DefaultBranch),
        "branch" => Ok(ServiceRefKind::Branch),
        "tag" => Ok(ServiceRefKind::Tag),
        "commit" => Ok(ServiceRefKind::Commit),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn specification_ref_kind_name(value: SpecificationRefKind) -> &'static str {
    match value {
        SpecificationRefKind::Branch => "branch",
        SpecificationRefKind::Tag => "tag",
        SpecificationRefKind::Commit => "commit",
    }
}
fn parse_specification_ref_kind(value: &str) -> Result<SpecificationRefKind, rusqlite::Error> {
    match value {
        "branch" => Ok(SpecificationRefKind::Branch),
        "tag" => Ok(SpecificationRefKind::Tag),
        "commit" => Ok(SpecificationRefKind::Commit),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn parse_binding_role(value: &str) -> Result<CodeBindingRole, rusqlite::Error> {
    match value {
        "primary" => Ok(CodeBindingRole::Primary),
        "additional" => Ok(CodeBindingRole::Additional),
        _ => Err(rusqlite::Error::InvalidQuery),
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
fn parse_entity_kind(value: &str) -> Result<CodeEntityKind, rusqlite::Error> {
    match value {
        "repository_root" => Ok(CodeEntityKind::RepositoryRoot),
        "directory" => Ok(CodeEntityKind::Directory),
        "file" => Ok(CodeEntityKind::File),
        "pattern" => Ok(CodeEntityKind::Pattern),
        _ => Err(rusqlite::Error::InvalidQuery),
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
fn parse_revision_kind(value: &str) -> Result<CodeRevisionKind, rusqlite::Error> {
    match value {
        "default_branch" => Ok(CodeRevisionKind::DefaultBranch),
        "branch" => Ok(CodeRevisionKind::Branch),
        "tag" => Ok(CodeRevisionKind::Tag),
        "commit" => Ok(CodeRevisionKind::Commit),
        "working_tree" => Ok(CodeRevisionKind::WorkingTree),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_kind(kind: &str) -> rusqlite::Result<SpecificationKind> {
    match kind {
        "markdown" => Ok(SpecificationKind::Markdown),
        "openapi" => Ok(SpecificationKind::OpenApi),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn controller_task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ControllerTask> {
    Ok(ControllerTask {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        workspace_id: row.get(2)?,
        controller_id: row.get(3)?,
        resource_kind: row.get(4)?,
        resource_id: row.get(5)?,
        generation: row.get(6)?,
        deduplication_key: row.get(7)?,
        status: row.get(8)?,
        attempts: row.get(9)?,
        next_run_at: row.get(10)?,
        lease_until: row.get(11)?,
        last_error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn repository_schedule_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepositorySyncSchedule> {
    Ok(RepositorySyncSchedule {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        workspace_id: row.get(2)?,
        repository_id: row.get(3)?,
        ref_kind: row.get(4)?,
        ref_name: row.get(5)?,
        interval_seconds: row.get(6)?,
        enabled: row.get(7)?,
        next_run_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn content_hash(content: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(content.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

fn file_sha256(path: &Path) -> Result<String, PersistenceError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoreError, Service};

    fn context(workspace_id: &str) -> WorkspaceContext {
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: workspace_id.into(), actor_id: "actor-1".into() }
    }

    #[test]
    fn migration_is_idempotent_and_reopen_preserves_facts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("specx.db");
        let context = context("workspace-1");
        let mut core = Core::default();
        core.create_group(&context, "root".into(), "Root".into(), None).unwrap();
        core.create_group(&context, "child".into(), "Child".into(), Some("root".into())).unwrap();
        core.create_tag(&context, "tag-a".into(), "Critical".into(), "#c00".into()).unwrap();
        core.create_service(
            &context,
            Service {
                id: "svc-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: Some("root".into()),
                name: "Gateway".into(),
                slug: "gateway".into(),
                description: "edge".into(),
                tag_ids: vec!["tag-a".into()],
                updated_at: "2026-10-23T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        core.create_markdown_specification(
            &context,
            Specification {
                id: "spec-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                service_id: "svc-1".into(),
                title: "README".into(),
                kind: SpecificationKind::Markdown,
                content: "# Gateway".into(),
                version: 1,
                content_hash: String::new(),
                updated_at: "2026-10-23T00:00:00Z".into(),
            },
        )
        .unwrap();
        {
            let store = SqliteStore::open(&path).unwrap();
            assert!(store.has_migration(1).unwrap());
            assert!(store.has_migration(5).unwrap());
            assert!(store.has_migration(8).unwrap());
            assert!(store.has_migration(9).unwrap());
            assert!(store.has_migration(10).unwrap());
            core.persist(&store, &context).unwrap();
            core.persist(&store, &context).unwrap();
            assert_eq!(store.specification_revision_count(&context, "spec-1").unwrap(), 1);
            let revision = store.specification_revision(&context, "spec-1", 1).unwrap().unwrap();
            assert_eq!(revision.content, "# Gateway");
            assert_eq!(revision.created_by, context.actor_id);
            assert_eq!(store.specification_revisions(&context, "spec-1", 10, 0).unwrap(), vec![revision]);
            let other_context = WorkspaceContext { workspace_id: "other-workspace".into(), ..context.clone() };
            assert!(store.specification_revision(&other_context, "spec-1", 1).unwrap().is_none());
        }
        let mut store = SqliteStore::open(&path).unwrap();
        let mut loaded = Core::load(&store, &context).unwrap();
        assert_eq!(loaded.list_services(&context).len(), 1);
        assert_eq!(loaded.list_groups(&context).len(), 2);
        assert!(!loaded.list_services(&context)[0].archived);
        loaded.set_service_archived(&context, "svc-1", true).unwrap();
        loaded.persist(&store, &context).unwrap();
        loaded = Core::load(&store, &context).unwrap();
        assert!(loaded.list_services(&context)[0].archived);
        let backup_path = directory.path().join("specx-backup.sqlite");
        store.backup_to_path(&backup_path).unwrap();
        let mut changed = Core::load(&store, &context).unwrap();
        changed
            .update_service(
                &context,
                "svc-1",
                "Gateway v2".into(),
                "gateway".into(),
                "changed after backup".into(),
                Some("root".into()),
                vec!["tag-a".into()],
            )
            .unwrap();
        changed.persist(&store, &context).unwrap();
        store.restore_from_path(&backup_path).unwrap();
        loaded = Core::load(&store, &context).unwrap();
        assert_eq!(loaded.list_services(&context)[0].name, "Gateway");
        assert!(loaded.list_services(&context)[0].archived);
        assert_eq!(
            loaded.specifications.get("spec-1").unwrap().content_hash,
            "sha256:247e06d69fc8bfaa58b4748636aa85808ac46c42ae0afe48a2fce06a8086628b"
        );

        let concurrent_store = SqliteStore::open(&path).unwrap();
        let mut updated = loaded;
        let mut concurrent = Core::load(&concurrent_store, &context).unwrap();
        let expected = updated.specifications.get("spec-1").unwrap().clone();
        let concurrent_expected = concurrent.specifications.get("spec-1").unwrap().clone();
        let update = updated
            .update_markdown_specification(&context, "spec-1", "README".into(), "# Updated Gateway".into())
            .unwrap();
        let concurrent_update = concurrent
            .update_markdown_specification(&context, "spec-1", "README".into(), "# Concurrent Gateway".into())
            .unwrap();
        assert!(store.update_specification_if_current(&context, &expected, &update).unwrap());
        assert!(
            !concurrent_store
                .update_specification_if_current(&context, &concurrent_expected, &concurrent_update)
                .unwrap()
        );
        assert_eq!(store.specification_revision_count(&context, "spec-1").unwrap(), 2);
        let updated = Core::load(&store, &context).unwrap();
        assert_eq!(updated.specifications.get("spec-1").unwrap().version, 2);
        assert_eq!(updated.specifications.get("spec-1").unwrap().content, "# Updated Gateway");
    }

    #[test]
    fn repository_snapshots_round_trip_and_keep_workspace_scope() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("specx.sqlite");
        let store = SqliteStore::open(&database).unwrap();
        let context = WorkspaceContext {
            tenant_id: "tenant-1".into(),
            workspace_id: "workspace-1".into(),
            actor_id: "actor-1".into(),
        };
        let other = WorkspaceContext {
            tenant_id: "tenant-1".into(),
            workspace_id: "workspace-2".into(),
            actor_id: "actor-1".into(),
        };
        let mut core = Core::default();
        let repository = core.create_repository(&context, "repo-1".into(), "https://github.com/acme/api").unwrap();
        let snapshot = RepositorySnapshot {
            id: "snapshot-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            repository_id: repository.id.clone(),
            source_ref: "main".into(),
            resolved_commit: "a".repeat(40),
            manifest_hash: "sha256:manifest".into(),
            content_root_ref: None,
            fetched_at: "2026-11-07T00:00:00Z".into(),
            provider_version: "git-cli".into(),
            created_at: "2026-11-07T00:00:00Z".into(),
        };
        core.record_repository_snapshot(&context, snapshot.clone()).unwrap();
        store.persist_core(&core, &context).unwrap();
        let reopened = SqliteStore::open(&database).unwrap();
        let loaded = reopened.load_core(&context).unwrap();
        assert_eq!(loaded.list_repository_snapshots(&context, &repository.id), vec![&snapshot]);
        assert!(loaded.list_repository_snapshots(&other, &repository.id).is_empty());
    }

    #[test]
    fn migration_backfills_one_repository_and_all_matching_legacy_refs_without_dropping_old_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-bindings.db");
        let context = context("workspace-1");
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-legacy".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Legacy service".into(),
                slug: "legacy-service".into(),
                description: String::new(),
                tag_ids: vec![],
                updated_at: "2026-11-01T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        let store = SqliteStore::open(&path).unwrap();
        store.persist_core(&core, &context).unwrap();
        for (id, key) in [("source-a", "legacy:one"), ("source-b", "legacy:two")] {
            store.connection.execute(
                "INSERT INTO code_sources(id,tenant_id,workspace_id,source_type,provider,display_name,canonical_key,root_locator,execution_scope,created_at,updated_at,archived)
                 VALUES(?1,?2,?3,'git','github','platform',?4,'https://github.com/acme/platform','server','2026-01-01','2026-01-01',0)",
                params![id, context.tenant_id, context.workspace_id, key],
            ).unwrap();
        }
        for (id, source_id, role, kind, path, revision_kind, revision, created_at) in [
            ("binding-main", "source-a", "primary", "repository_root", "", "branch", Some("main"), "2026-01-01"),
            (
                "binding-release",
                "source-b",
                "additional",
                "directory",
                "services/legacy",
                "branch",
                Some("release/1.x"),
                "2026-01-02",
            ),
            (
                "binding-tag",
                "source-b",
                "additional",
                "directory",
                "services/legacy/release",
                "tag",
                Some("v1.2.0"),
                "2026-01-03",
            ),
        ] {
            store.connection.execute(
                "INSERT INTO service_code_bindings(id,tenant_id,workspace_id,service_id,source_id,role,entity_kind,path,revision_kind,revision,created_at,updated_at)
                 VALUES(?1,?2,?3,'svc-legacy',?4,?5,?6,?7,?8,?9,?10,?10)",
                params![id, context.tenant_id, context.workspace_id, source_id, role, kind, path, revision_kind, revision, created_at],
            ).unwrap();
        }
        drop(store);

        let migrated = SqliteStore::open(&path).unwrap();
        let loaded = migrated.load_core(&context).unwrap();
        let relation = loaded.service_repository(&context, "svc-legacy").unwrap();
        assert_eq!(relation.id, "binding-main");
        let repository = loaded.repository(&context, &relation.repository_id).unwrap();
        assert_eq!(repository.canonical_url, "https://github.com/acme/platform");
        let mut refs: Vec<_> = loaded
            .list_service_ref_tracks(&context, &relation.id)
            .iter()
            .map(|track| (track.ref_kind, track.ref_name.as_deref()))
            .collect();
        refs.sort_by_key(|(kind, name)| (*kind, *name));
        assert_eq!(
            refs,
            vec![
                (ServiceRefKind::Branch, Some("main")),
                (ServiceRefKind::Branch, Some("release/1.x")),
                (ServiceRefKind::Tag, Some("v1.2.0")),
            ]
        );
        let old_binding_count: i64 = migrated
            .connection
            .query_row("SELECT COUNT(*) FROM service_code_bindings WHERE service_id='svc-legacy'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(old_binding_count, 3);
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_markers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("broken-migration.db");
        let connection = Connection::open(&path).unwrap();
        // This deliberately collides with the v1 table name while omitting the
        // columns required by the v7 workspace backfill. The migration must
        // leave the file exactly as it was when that backfill fails.
        connection.execute("CREATE TABLE workspaces (tenant_id TEXT NOT NULL)", []).unwrap();
        drop(connection);

        assert!(SqliteStore::open(&path).is_err());

        let connection = Connection::open(&path).unwrap();
        let migrations_table: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!migrations_table);
        let services_columns = connection
            .prepare("PRAGMA table_info(services)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(services_columns.is_empty());
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn service_compare_and_swap_allows_only_one_writer_across_connections() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("service-cas.sqlite");
        let context = context("workspace-1");
        let store = SqliteStore::open(&path).unwrap();
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Gateway".into(),
                slug: "gateway".into(),
                description: "edge".into(),
                tag_ids: vec![],
                updated_at: "2026-10-24T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        store.persist_core(&core, &context).unwrap();
        let peer = SqliteStore::open(&path).unwrap();
        let expected = store.load_core(&context).unwrap().list_services(&context)[0].clone();
        let peer_expected = peer.load_core(&context).unwrap().list_services(&context)[0].clone();
        let mut first_update = expected.clone();
        first_update.name = "First writer".into();
        first_update.description = "committed".into();
        first_update.updated_at = "2026-10-25T00:00:00Z".into();
        let mut stale_update = peer_expected.clone();
        stale_update.name = "Stale writer".into();
        stale_update.description = "must not overwrite".into();
        stale_update.updated_at = "2026-10-25T00:00:01Z".into();
        let audit = AuditEvent {
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            actor_id: context.actor_id.clone(),
            request_id: "service-cas".into(),
            action: "update".into(),
            resource_type: "service".into(),
            resource_id: "svc-1".into(),
            created_at: "2026-10-25T00:00:00Z".into(),
        };

        assert!(store.update_service_if_current_with_audit(&context, &expected, &first_update, Some(&audit)).unwrap());
        assert!(
            !peer.update_service_if_current_with_audit(&context, &peer_expected, &stale_update, Some(&audit)).unwrap()
        );
        let persisted = store.load_core(&context).unwrap().list_services(&context)[0].clone();
        assert_eq!(persisted.name, "First writer");
        assert_eq!(persisted.description, "committed");
        assert_eq!(store.audit_event_count(&context).unwrap(), 1);
    }

    #[test]
    fn code_source_and_binding_compare_and_swap_allow_only_one_writer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("code-cas.sqlite");
        let context = context("workspace-1");
        let store = SqliteStore::open(&path).unwrap();
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-code".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Payments".into(),
                slug: "payments".into(),
                description: String::new(),
                tag_ids: vec![],
                updated_at: "2026-10-24T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        core.create_code_source(
            &context,
            CodeSource {
                id: "src-code".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                source_type: CodeSourceType::Git,
                provider: "github".into(),
                display_name: "acme/payments".into(),
                canonical_key: "github:acme/payments".into(),
                root_locator: "https://github.com/acme/payments.git".into(),
                execution_scope: CodeExecutionScope::Server,
                created_at: "2026-10-24T00:00:00Z".into(),
                updated_at: "2026-10-24T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        core.create_code_binding(
            &context,
            ServiceCodeBinding {
                id: "binding-code".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                service_id: "svc-code".into(),
                source_id: "src-code".into(),
                role: CodeBindingRole::Primary,
                entity_kind: CodeEntityKind::Directory,
                path: "services/payments".into(),
                revision_kind: CodeRevisionKind::Branch,
                revision: Some("main".into()),
                created_at: "2026-10-24T00:00:00Z".into(),
                updated_at: "2026-10-24T00:00:00Z".into(),
            },
        )
        .unwrap();
        store.persist_core(&core, &context).unwrap();

        let peer = SqliteStore::open(&path).unwrap();
        let first = store.load_core(&context).unwrap();
        let second = peer.load_core(&context).unwrap();
        let expected_source = first.list_code_sources(&context)[0].clone();
        let peer_expected_source = second.list_code_sources(&context)[0].clone();
        let mut updated_source = expected_source.clone();
        updated_source.provider = "gitlab".into();
        updated_source.updated_at = "2026-10-25T00:00:00Z".into();
        let mut stale_source = peer_expected_source.clone();
        stale_source.provider = "bitbucket".into();
        stale_source.updated_at = "2026-10-25T00:00:01Z".into();

        assert!(
            store.update_code_source_if_current_with_audit(&context, &expected_source, &updated_source, None).unwrap()
        );
        assert!(
            !peer
                .update_code_source_if_current_with_audit(&context, &peer_expected_source, &stale_source, None)
                .unwrap()
        );

        let expected_binding = first.list_service_code_bindings(&context, "svc-code")[0].clone();
        let peer_expected_binding = second.list_service_code_bindings(&context, "svc-code")[0].clone();
        let mut updated_binding = expected_binding.clone();
        updated_binding.path = "services/payments/api".into();
        updated_binding.updated_at = "2026-10-25T00:01:00Z".into();
        let mut stale_binding = peer_expected_binding.clone();
        stale_binding.path = "services/payments/stale".into();
        stale_binding.updated_at = "2026-10-25T00:01:01Z".into();

        assert!(
            store
                .update_code_binding_if_current_with_audit(&context, &expected_binding, &updated_binding, None)
                .unwrap()
        );
        assert!(
            !peer
                .update_code_binding_if_current_with_audit(&context, &peer_expected_binding, &stale_binding, None)
                .unwrap()
        );
        let persisted = store.load_core(&context).unwrap();
        assert_eq!(persisted.list_code_sources(&context)[0].provider, "gitlab");
        assert_eq!(persisted.list_service_code_bindings(&context, "svc-code")[0].path, "services/payments/api");
    }

    #[test]
    fn migration_v4_adds_archived_state_without_changing_existing_services() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("specx-v4.db");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 INSERT INTO schema_migrations(version, applied_at) VALUES (1, datetime('now')), (2, datetime('now')), (3, datetime('now')), (4, datetime('now'));
                 CREATE TABLE services (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   group_id TEXT, name TEXT NOT NULL, slug TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
                   updated_at TEXT NOT NULL DEFAULT '', UNIQUE (tenant_id, workspace_id, slug)
                 );
                 INSERT INTO services(id, tenant_id, workspace_id, name, slug) VALUES ('svc-1', 'tenant-1', 'workspace-1', 'Gateway', 'gateway');",
            )
            .unwrap();
        drop(legacy);

        let store = SqliteStore::open(&path).unwrap();
        let context = context("workspace-1");
        assert!(store.has_migration(5).unwrap());
        let loaded = Core::load(&store, &context).unwrap();
        let service = loaded.list_services(&context).into_iter().find(|service| service.id == "svc-1").unwrap();
        assert!(!service.archived);
    }

    #[test]
    fn migration_v8_adds_invitation_revocation_without_losing_existing_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("specx-v8.db");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 INSERT INTO schema_migrations(version, applied_at)
                   VALUES (1, datetime('now')), (2, datetime('now')), (3, datetime('now')), (4, datetime('now')),
                          (5, datetime('now')), (6, datetime('now')), (7, datetime('now')), (8, datetime('now'));
                 CREATE TABLE workspace_invitations (
                   id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                   email TEXT NOT NULL, role TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
                   expires_at TEXT NOT NULL, accepted_at TEXT, created_by TEXT NOT NULL, created_at TEXT NOT NULL
                 );
                 INSERT INTO workspace_invitations(id, tenant_id, workspace_id, email, role, token_hash, expires_at, created_by, created_at)
                   VALUES ('invite-legacy', 'tenant-1', 'default', 'legacy@example.com', 'viewer', 'sha256:legacy',
                           '2026-11-01T00:00:00+00:00', 'owner', '2026-10-25T00:00:00+00:00');",
            )
            .unwrap();
        drop(legacy);

        let store = SqliteStore::open(&path).unwrap();
        assert!(store.has_migration(9).unwrap());
        assert!(store.has_migration(10).unwrap());
        let invitation = store.find_workspace_invitation("sha256:legacy").unwrap().unwrap();
        assert_eq!(invitation.email, "legacy@example.com");
        assert_eq!(invitation.revoked_at, None);
    }

    #[test]
    fn expired_pending_invitations_are_purged_and_history_is_retained() {
        let store = SqliteStore::open_in_memory().unwrap();
        let has_expiry_index: bool = store
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_workspace_invitations_expires_at')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_expiry_index);
        store
            .connection
            .execute_batch(
                "INSERT INTO workspace_invitations(id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at) VALUES
                 ('expired-pending','tenant-1','default','expired@example.com','viewer','hash-expired','2026-01-01T00:00:00Z',NULL,NULL,'owner','2025-01-01T00:00:00Z'),
                 ('expired-accepted','tenant-1','default','accepted@example.com','viewer','hash-accepted','2026-01-01T00:00:00Z','2025-12-01T00:00:00Z',NULL,'owner','2025-01-01T00:00:00Z'),
                 ('expired-revoked','tenant-1','default','revoked@example.com','viewer','hash-revoked','2026-01-01T00:00:00Z',NULL,'2025-12-01T00:00:00Z','owner','2025-01-01T00:00:00Z'),
                 ('unexpired-pending','tenant-1','default','pending@example.com','viewer','hash-pending','2027-01-01T00:00:00Z',NULL,NULL,'owner','2025-01-01T00:00:00Z');",
            )
            .unwrap();

        assert_eq!(store.purge_expired_workspace_invitations("2026-02-01T00:00:00Z").unwrap(), 1);
        assert!(store.find_workspace_invitation("hash-expired").unwrap().is_none());
        assert!(store.find_workspace_invitation("hash-accepted").unwrap().is_some());
        assert!(store.find_workspace_invitation("hash-revoked").unwrap().is_some());
        assert!(store.find_workspace_invitation("hash-pending").unwrap().is_some());
    }

    #[test]
    fn scheduled_backups_are_integrity_checked_and_rotated() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("specx.sqlite");
        let backup_directory = directory.path().join("backups");
        let store = SqliteStore::open(&database).unwrap();

        let first = store.create_backup(&backup_directory, "specx", 2).unwrap();
        assert!(first.path.exists());
        assert!(first.bytes > 0);
        assert!(first.sha256.starts_with("sha256:"));
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = store.create_backup(&backup_directory, "specx", 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let third = store.create_backup(&backup_directory, "specx", 2).unwrap();
        assert_ne!(first.path, second.path);
        assert_ne!(second.path, third.path);
        let retained = std::fs::read_dir(&backup_directory).unwrap().count();
        assert_eq!(retained, 2);
        assert!(!first.path.exists());
        assert!(third.path.exists());
        assert!(store.create_backup(&backup_directory, "../escape", 2).is_err());
        assert!(store.create_backup(&backup_directory, "specx", 0).is_err());
    }

    #[test]
    fn workspace_backup_restore_preserves_other_workspace_facts() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("specx.sqlite");
        let backup_directory = directory.path().join("backups");
        let store = SqliteStore::open(&database).unwrap();
        let workspace_one = context("workspace-one");
        let mut workspace_two = context("workspace-two");
        workspace_two.actor_id = "actor-two".into();
        let mut core = Core::default();
        for (context, id, name) in
            [(&workspace_one, "svc-one", "Workspace One"), (&workspace_two, "svc-two", "Workspace Two")]
        {
            core.create_service(
                context,
                Service {
                    id: id.into(),
                    tenant_id: context.tenant_id.clone(),
                    workspace_id: context.workspace_id.clone(),
                    group_id: None,
                    name: name.into(),
                    slug: id.into(),
                    description: String::new(),
                    tag_ids: Vec::new(),
                    updated_at: Utc::now().to_rfc3339(),
                    archived: false,
                },
            )
            .unwrap();
        }
        store.persist_core(&core, &workspace_one).unwrap();
        store.persist_core(&core, &workspace_two).unwrap();
        let prefix = "workspace-one";
        let backup = store.create_workspace_backup(&workspace_one, &backup_directory, prefix, 3).unwrap();

        let mut changed_one = Core::load(&store, &workspace_one).unwrap();
        changed_one
            .update_service(
                &workspace_one,
                "svc-one",
                "Changed One".into(),
                "svc-one".into(),
                String::new(),
                None,
                Vec::new(),
            )
            .unwrap();
        store.persist_core(&changed_one, &workspace_one).unwrap();
        let mut changed_two = Core::load(&store, &workspace_two).unwrap();
        changed_two
            .update_service(
                &workspace_two,
                "svc-two",
                "Changed Two".into(),
                "svc-two".into(),
                String::new(),
                None,
                Vec::new(),
            )
            .unwrap();
        store.persist_core(&changed_two, &workspace_two).unwrap();

        let mut store = SqliteStore::open(&database).unwrap();
        store.restore_workspace_from_path(&workspace_one, &backup.path).unwrap();
        let restored_one = Core::load(&store, &workspace_one).unwrap();
        let restored_two = Core::load(&store, &workspace_two).unwrap();
        assert_eq!(restored_one.list_services(&workspace_one)[0].name, "Workspace One");
        assert_eq!(restored_two.list_services(&workspace_two)[0].name, "Changed Two");
    }

    #[test]
    fn persisted_scope_does_not_cross_workspaces() {
        let store = SqliteStore::open_in_memory().unwrap();
        let one = context("workspace-1");
        let two = context("workspace-2");
        let mut core = Core::default();
        core.create_group(&one, "root".into(), "Root".into(), None).unwrap();
        core.persist(&store, &one).unwrap();
        assert!(Core::load(&store, &two).unwrap().list_services(&two).is_empty());
        assert_eq!(
            core.create_group(&two, "bad".into(), "Bad".into(), Some("root".into())),
            Err(CoreError::ScopeMismatch)
        );
    }

    #[test]
    fn workspace_membership_bootstraps_once_and_supports_revocation() {
        let store = SqliteStore::open_in_memory().unwrap();
        let owner = context("default");
        let mut member = owner.clone();
        member.actor_id = "actor-2".into();
        assert!(store.authorize_workspace(&owner).unwrap());
        assert!(!store.authorize_workspace(&member).unwrap());
        assert!(store.revoke_workspace_member(&owner.tenant_id, &owner.workspace_id, &owner.actor_id).unwrap());
        assert!(!store.authorize_workspace(&member).unwrap());
    }

    #[test]
    fn invitation_acceptance_rolls_back_membership_and_consumption_when_audit_fails() {
        let store = SqliteStore::open_in_memory().unwrap();
        let owner = context("default");
        assert!(store.authorize_workspace(&owner).unwrap());
        let invitation = WorkspaceInvitation {
            id: "invite-1".into(),
            tenant_id: owner.tenant_id.clone(),
            workspace_id: owner.workspace_id.clone(),
            email: "member@example.com".into(),
            role: "editor".into(),
            token_hash: "sha256:opaque-token-hash".into(),
            expires_at: "2026-11-01T00:00:00+00:00".into(),
            accepted_at: None,
            revoked_at: None,
            created_by: owner.actor_id.clone(),
            created_at: "2026-10-25T00:00:00+00:00".into(),
        };
        let invite_audit = AuditEvent {
            tenant_id: owner.tenant_id.clone(),
            workspace_id: owner.workspace_id.clone(),
            actor_id: owner.actor_id.clone(),
            request_id: "invite-create".into(),
            action: "invite".into(),
            resource_type: "workspace_invitation".into(),
            resource_id: invitation.id.clone(),
            created_at: invitation.created_at.clone(),
        };
        assert!(store.create_workspace_invitation(&owner, &invitation, &invite_audit).unwrap());
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER fail_invitation_audit BEFORE INSERT ON audit_events
                 WHEN NEW.action = 'accept_invitation'
                 BEGIN SELECT RAISE(ABORT, 'simulated audit failure'); END;",
            )
            .unwrap();

        let mut member = owner.clone();
        member.actor_id = "actor-2".into();
        let acceptance_audit = AuditEvent {
            tenant_id: member.tenant_id.clone(),
            workspace_id: member.workspace_id.clone(),
            actor_id: member.actor_id.clone(),
            request_id: "invite-accept".into(),
            action: "accept_invitation".into(),
            resource_type: "workspace_invitation".into(),
            resource_id: invitation.id.clone(),
            created_at: "2026-10-25T00:00:01+00:00".into(),
        };
        assert!(
            store
                .accept_workspace_invitation(
                    &member,
                    &invitation.token_hash,
                    "2026-10-25T00:00:01+00:00",
                    &acceptance_audit
                )
                .is_err()
        );
        assert_eq!(store.workspace_member_role(&member, "actor-2").unwrap(), None);
        assert_eq!(store.find_workspace_invitation(&invitation.token_hash).unwrap().unwrap().accepted_at, None);
        assert_eq!(store.audit_event_count(&owner).unwrap(), 1);

        store.connection.execute_batch("DROP TRIGGER fail_invitation_audit;").unwrap();
        assert!(
            store
                .accept_workspace_invitation(
                    &member,
                    &invitation.token_hash,
                    "2026-10-25T00:00:02+00:00",
                    &acceptance_audit
                )
                .unwrap()
                .is_some()
        );
        assert_eq!(store.workspace_member_role(&member, "actor-2").unwrap().as_deref(), Some("editor"));
        assert_eq!(
            store.find_workspace_invitation(&invitation.token_hash).unwrap().unwrap().accepted_at.as_deref(),
            Some("2026-10-25T00:00:02+00:00")
        );
        assert_eq!(store.audit_event_count(&owner).unwrap(), 2);
        assert!(
            store
                .accept_workspace_invitation(
                    &member,
                    &invitation.token_hash,
                    "2026-10-25T00:00:03+00:00",
                    &acceptance_audit
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn workspace_metadata_is_scoped_persisted_and_created_with_owner_membership() {
        let store = SqliteStore::open_in_memory().unwrap();
        let owner = context("default");
        assert!(store.authorize_workspace(&owner).unwrap());
        assert_eq!(store.workspaces_for_actor("tenant-1", "actor-1").unwrap()[0].workspace.name, "默认工作区");

        let workspace = Workspace {
            id: "workspace-new".into(),
            tenant_id: owner.tenant_id.clone(),
            name: "New workspace".into(),
            slug: "new-workspace-a1".into(),
            created_at: Utc::now().to_rfc3339(),
        };
        let audit = AuditEvent {
            tenant_id: owner.tenant_id.clone(),
            workspace_id: workspace.id.clone(),
            actor_id: owner.actor_id.clone(),
            request_id: "request-1".into(),
            action: "create".into(),
            resource_type: "workspace".into(),
            resource_id: workspace.id.clone(),
            created_at: workspace.created_at.clone(),
        };
        assert!(store.create_workspace(&owner, &workspace, &audit).unwrap());
        let workspaces = store.workspaces_for_actor("tenant-1", "actor-1").unwrap();
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces.iter().any(|item| item.workspace.id == workspace.id
            && item.workspace.name == workspace.name
            && item.role == "owner"));
        assert_eq!(
            store
                .workspace_member_role(
                    &WorkspaceContext { workspace_id: workspace.id.clone(), ..owner.clone() },
                    "actor-1"
                )
                .unwrap()
                .as_deref(),
            Some("owner")
        );

        let mut editor = owner.clone();
        editor.actor_id = "actor-2".into();
        let member_audit =
            AuditEvent { actor_id: owner.actor_id.clone(), workspace_id: owner.workspace_id.clone(), ..audit };
        assert!(store.set_workspace_member_role(&owner, &editor.actor_id, "editor", &member_audit).unwrap());
        assert!(
            !store
                .create_workspace(
                    &editor,
                    &Workspace { id: "workspace-denied".into(), ..workspace.clone() },
                    &member_audit
                )
                .unwrap()
        );
        assert_eq!(store.workspaces_for_actor("tenant-1", &editor.actor_id).unwrap().len(), 1);
    }

    #[test]
    fn failed_audit_insert_rolls_back_catalog_write() {
        let store = SqliteStore::open_in_memory().unwrap();
        let context = context("default");
        let mut core = Core::default();
        core.create_group(&context, "root".into(), "Root".into(), None).unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_audit BEFORE INSERT ON audit_events
                 BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        let event = AuditEvent {
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            actor_id: context.actor_id.clone(),
            request_id: "request-1".into(),
            action: "create".into(),
            resource_type: "service_group".into(),
            resource_id: "root".into(),
            created_at: Utc::now().to_rfc3339(),
        };

        assert!(store.persist_core_with_audit(&core, &context, Some(&event)).is_err());
        assert!(Core::load(&store, &context).unwrap().list_groups(&context).is_empty());
        assert_eq!(store.audit_event_count(&context).unwrap(), 0);
    }

    #[test]
    fn specification_and_snapshot_commit_roll_back_together() {
        let store = SqliteStore::open_in_memory().unwrap();
        let context = context("default");
        let mut core = Core::default();
        core.create_group(&context, "root".into(), "Root".into(), None).unwrap();
        core.create_service(
            &context,
            Service {
                id: "service-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: Some("root".into()),
                name: "Gateway".into(),
                slug: "gateway".into(),
                description: String::new(),
                tag_ids: Vec::new(),
                updated_at: "2026-11-08T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        core.create_markdown_specification(
            &context,
            Specification {
                id: "spec-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                service_id: "service-1".into(),
                title: "README".into(),
                kind: SpecificationKind::Markdown,
                content: "# Before".into(),
                version: 1,
                content_hash: String::new(),
                updated_at: "2026-11-08T00:00:00Z".into(),
            },
        )
        .unwrap();
        core.persist(&store, &context).unwrap();

        let expected = Core::load(&store, &context).unwrap().specifications.get("spec-1").unwrap().clone();
        let mut updated = expected.clone();
        updated.content = "# After".into();
        updated.updated_at = "2026-11-08T00:01:00Z".into();
        let snapshot = RepositorySnapshot {
            id: "snapshot-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            repository_id: "missing-repository".into(),
            source_ref: "main".into(),
            resolved_commit: "a".repeat(40),
            manifest_hash: "sha256:manifest".into(),
            content_root_ref: None,
            fetched_at: updated.updated_at.clone(),
            provider_version: "git-cli".into(),
            created_at: updated.updated_at.clone(),
        };

        assert!(
            store
                .update_specification_if_current_with_audit_and_snapshot(
                    &context,
                    &expected,
                    &updated,
                    None,
                    Some(&snapshot),
                )
                .is_err()
        );
        let restored = Core::load(&store, &context).unwrap();
        assert_eq!(restored.specifications.get("spec-1").unwrap(), &expected);
        assert_eq!(store.specification_revision_count(&context, "spec-1").unwrap(), 1);
        assert!(restored.list_repository_snapshots(&context, "missing-repository").is_empty());
    }

    #[test]
    fn rate_limit_is_shared_across_sqlite_connections_and_resets_by_window() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("specx.db");
        let first = SqliteStore::open(&path).unwrap();
        let second = SqliteStore::open(&path).unwrap();
        let context = context("default");

        assert!(first.consume_rate_limit(&context, 100, 60, 1).unwrap());
        assert!(!second.consume_rate_limit(&context, 120, 60, 1).unwrap());
        assert!(second.consume_rate_limit(&context, 160, 60, 1).unwrap());
    }

    #[test]
    fn same_workspace_id_is_isolated_between_tenants() {
        let store = SqliteStore::open_in_memory().unwrap();
        let tenant_one = WorkspaceContext {
            tenant_id: "tenant-1".into(),
            workspace_id: "shared".into(),
            actor_id: "actor-1".into(),
        };
        let tenant_two = WorkspaceContext {
            tenant_id: "tenant-2".into(),
            workspace_id: "shared".into(),
            actor_id: "actor-2".into(),
        };
        let mut core = Core::default();
        core.create_group(&tenant_one, "root-1".into(), "Tenant one".into(), None).unwrap();
        core.create_group(&tenant_two, "root-2".into(), "Tenant two".into(), None).unwrap();
        core.persist(&store, &tenant_one).unwrap();
        core.persist(&store, &tenant_two).unwrap();
        assert_eq!(
            Core::load(&store, &tenant_one)
                .unwrap()
                .list_groups(&tenant_one)
                .iter()
                .map(|group| group.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-1"]
        );
        assert_eq!(
            Core::load(&store, &tenant_two)
                .unwrap()
                .list_groups(&tenant_two)
                .iter()
                .map(|group| group.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-2"]
        );
    }

    #[test]
    fn web_sessions_are_durable_and_revocable_without_persisting_bearers() {
        let store = SqliteStore::open_in_memory().unwrap();
        let session = WebSession {
            token_hash: "sha256:session-one".into(),
            actor_id: "actor-one".into(),
            expires_at: "2026-10-25T01:00:00Z".into(),
            revoked_at: None,
            created_at: "2026-10-25T00:00:00Z".into(),
        };
        store.create_web_session(&session).unwrap();
        assert!(store.web_session_is_active(&session.token_hash, "2026-10-25T00:30:00Z").unwrap());
        assert!(!store.web_session_is_active(&session.token_hash, "2026-10-25T01:00:00Z").unwrap());
        store.create_web_session(&session).unwrap();
        assert!(store.revoke_web_session(&session.token_hash, "2026-10-25T00:40:00Z").unwrap());
        assert!(!store.web_session_is_active(&session.token_hash, "2026-10-25T00:41:00Z").unwrap());
    }
}
