//! Durable storage for the specX domain facts.
//!
//! This first adapter intentionally uses SQLite so the core can be exercised
//! without a running service. The repository shape and scoped queries are the
//! same contract a PostgreSQL adapter must implement for the production API.

use crate::{Core, Id, Service, ServiceGroup, Specification, SpecificationKind, Tag, Workspace, WorkspaceContext};

pub const LATEST_SCHEMA_VERSION: i64 = 11;
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
            "specification_revisions",
            "specifications",
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
        for table in [
            "service_groups",
            "tags",
            "services",
            "service_tags",
            "specifications",
            "specification_revisions",
            "workspace_members",
            "audit_events",
            "api_idempotency",
            "workspace_invitations",
        ] {
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
                "specification_revisions",
                "specifications",
                "services",
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
            for (table, columns) in [
                ("service_groups", "id, tenant_id, workspace_id, parent_id, name, position"),
                ("tags", "id, tenant_id, workspace_id, name, color"),
                ("services", "id, tenant_id, workspace_id, group_id, name, slug, description, updated_at, archived"),
                ("service_tags", "tenant_id, workspace_id, service_id, tag_id"),
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
            ] {
                transaction.execute_batch(&format!(
                    "INSERT INTO main.{table}({columns}) SELECT {columns} FROM workspace_backup.{table};"
                ))?;
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
        if expected.id != updated.id
            || expected.tenant_id != context.tenant_id
            || expected.workspace_id != context.workspace_id
            || updated.tenant_id != context.tenant_id
            || updated.workspace_id != context.workspace_id
        {
            return Err(PersistenceError("specification update scope does not match".into()));
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
    }
}

fn parse_kind(kind: &str) -> rusqlite::Result<SpecificationKind> {
    match kind {
        "markdown" => Ok(SpecificationKind::Markdown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
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
