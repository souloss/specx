//! specX HTTP boundary.
//!
//! The API owns transport concerns only. Domain invariants and durable writes
//! remain in `specx-core`; the specX Web host derives the request context from its
//! server-side session before this router is invoked.

mod backup_storage;
pub mod errors;
mod resources;

use errors::{ErrorKind, ErrorResponse};
pub use resources::{ApiDoc, CatalogResponse, SpecificationResponse, router};
use resources::{backup_file_name, backup_prefix};

use axum::middleware::{self, Next};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CONTENT_TYPE, ETAG, HOST, IF_MATCH, ORIGIN},
    },
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use backup_storage::{BackupObject, BackupStorage};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specx_core::persistence::{
    AuditEvent, BackupArtifact, IdempotencyRecord, LATEST_SCHEMA_VERSION, PersistenceError, SpecificationRevision,
    SqliteStore, WebSession, WorkspaceInvitation, WorkspaceMember, WorkspaceMembership,
};
#[cfg(feature = "postgres")]
use specx_core::postgres::PostgresStore;
use specx_core::{
    Core, CoreError, Id, Service, ServiceGroup, ServiceGroupOrder, Specification, SpecificationKind, Tag, Workspace,
    WorkspaceContext,
};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::Mutex;
use utoipa::OpenApi;

static REQUEST_ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn test_backup_root() -> PathBuf {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!("specx-api-backups-{}-{stamp}", std::process::id()))
}

#[derive(Clone)]
pub struct ApiState {
    inner: Arc<Mutex<ApiInner>>,
    backup_root: Arc<PathBuf>,
    backup_storage: Option<Arc<BackupStorage>>,
    backup_retention: usize,
    metrics: Arc<ApiMetrics>,
    postgres_backend: bool,
    public_origin: Option<PublicOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublicOrigin {
    scheme: String,
    authority: String,
}

#[derive(Default)]
struct ApiMetrics {
    requests_total: AtomicU64,
    backups_created_total: AtomicU64,
    backup_creation_failures_total: AtomicU64,
    backups_restored_total: AtomicU64,
    backup_restore_failures_total: AtomicU64,
    invitations_expired_total: AtomicU64,
    invitation_cleanup_failures_total: AtomicU64,
    ready: AtomicU64,
}

impl ApiMetrics {
    fn storage_ready() -> Self {
        Self { ready: AtomicU64::new(1), ..Self::default() }
    }
}

struct ApiInner {
    store: StoreBackend,
    seed_core: Option<Core>,
}

enum StoreBackend {
    Sqlite(SqliteStore),
    #[cfg(feature = "postgres")]
    Postgres(PostgresStore),
}

impl StoreBackend {
    fn sqlite(store: SqliteStore) -> Self {
        Self::Sqlite(store)
    }

    #[cfg(feature = "postgres")]
    fn postgres(store: PostgresStore) -> Self {
        Self::Postgres(store)
    }

    fn ready(&self) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.ready(),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => run_postgres_future(store.clone(), |store| async move { store.ready().await }),
        }
    }

    fn schema_version(&self) -> i64 {
        match self {
            Self::Sqlite(_) => LATEST_SCHEMA_VERSION,
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => LATEST_SCHEMA_VERSION,
        }
    }

    fn authorize_workspace(&self, context: &WorkspaceContext) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.authorize_workspace(context),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(
                    store.clone(),
                    move |store| async move { store.authorize_workspace(&context).await },
                )
            }
        }
    }

    fn workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
    ) -> Result<Option<String>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.workspace_member_role(context, actor_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let actor_id = actor_id.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.workspace_member_role(&context, &actor_id).await
                })
            }
        }
    }

    fn consume_rate_limit(
        &self,
        context: &WorkspaceContext,
        now: i64,
        window_seconds: i64,
        limit: u32,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.consume_rate_limit(context, now, window_seconds, limit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.consume_rate_limit(&context, now, window_seconds, limit).await
                })
            }
        }
    }

    fn load_core(&self, context: &WorkspaceContext) -> Result<Core, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.load_core(context),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(store.clone(), move |store| async move { store.load_core(&context).await })
            }
        }
    }

    fn persist_core_with_audit(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.persist_core_with_audit(core, context, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let core = core.clone();
                let context = context.clone();
                let audit = audit.cloned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.persist_core_with_audit(&core, &context, audit.as_ref()).await
                })
            }
        }
    }

    fn persist_core_with_audit_and_idempotency(
        &self,
        core: &Core,
        context: &WorkspaceContext,
        audit: Option<&AuditEvent>,
        idempotency: Option<(&str, &str, &str, &str, &str, &str)>,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.persist_core_with_audit_and_idempotency(core, context, audit, idempotency),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let core = core.clone();
                let context = context.clone();
                let audit = audit.cloned();
                let idempotency =
                    idempotency.map(|(key, tenant, workspace, fingerprint, resource_type, resource_id)| {
                        (
                            key.to_owned(),
                            tenant.to_owned(),
                            workspace.to_owned(),
                            fingerprint.to_owned(),
                            resource_type.to_owned(),
                            resource_id.to_owned(),
                        )
                    });
                run_postgres_future(store.clone(), move |store| async move {
                    let idempotency = idempotency.as_ref().map(|values| {
                        (
                            values.0.as_str(),
                            values.1.as_str(),
                            values.2.as_str(),
                            values.3.as_str(),
                            values.4.as_str(),
                            values.5.as_str(),
                        )
                    });
                    store.persist_core_with_audit_and_idempotency(&core, &context, audit.as_ref(), idempotency).await
                })
            }
        }
    }

    fn load_idempotency(&self, key: &str) -> Result<Option<IdempotencyRecord>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.load_idempotency(key),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let key = key.to_owned();
                run_postgres_future(store.clone(), move |store| async move { store.load_idempotency(&key).await })
            }
        }
    }

    fn workspace_members(&self, context: &WorkspaceContext) -> Result<Vec<WorkspaceMember>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.workspace_members(context),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(store.clone(), move |store| async move { store.workspace_members(&context).await })
            }
        }
    }

    fn create_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        invitation: &WorkspaceInvitation,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.create_workspace_invitation(context, invitation, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let store = store.clone();
                let context = context.clone();
                let invitation = invitation.clone();
                let audit = audit.clone();
                run_postgres_future(store, move |store| async move {
                    store.create_workspace_invitation(&context, &invitation, &audit).await
                })
            }
        }
    }

    fn workspace_invitations(&self, context: &WorkspaceContext) -> Result<Vec<WorkspaceInvitation>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.workspace_invitations(context),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(
                    store.clone(),
                    move |store| async move { store.workspace_invitations(&context).await },
                )
            }
        }
    }

    fn purge_expired_workspace_invitations(&self, now: &str) -> Result<u64, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.purge_expired_workspace_invitations(now),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let now = now.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.purge_expired_workspace_invitations(&now).await
                })
            }
        }
    }

    fn find_workspace_invitation(
        &self,
        tenant_id: &str,
        token_hash: &str,
    ) -> Result<Option<WorkspaceInvitation>, PersistenceError> {
        match self {
            Self::Sqlite(store) => {
                Ok(store.find_workspace_invitation(token_hash)?.filter(|invitation| invitation.tenant_id == tenant_id))
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let tenant_id = tenant_id.to_owned();
                let token_hash = token_hash.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.find_workspace_invitation(&tenant_id, &token_hash).await
                })
            }
        }
    }

    fn accept_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        token_hash: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<Option<WorkspaceMember>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.accept_workspace_invitation(context, token_hash, now, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let token_hash = token_hash.to_owned();
                let now = now.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.accept_workspace_invitation(&context, &token_hash, &now, &audit).await
                })
            }
        }
    }

    fn revoke_workspace_invitation(
        &self,
        context: &WorkspaceContext,
        invitation_id: &str,
        now: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.revoke_workspace_invitation(context, invitation_id, now, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let invitation_id = invitation_id.to_owned();
                let now = now.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.revoke_workspace_invitation(&context, &invitation_id, &now, &audit).await
                })
            }
        }
    }

    fn workspaces_for_actor(
        &self,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<Vec<WorkspaceMembership>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.workspaces_for_actor(tenant_id, actor_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let tenant_id = tenant_id.to_owned();
                let actor_id = actor_id.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.workspaces_for_actor(&tenant_id, &actor_id).await
                })
            }
        }
    }

    fn create_workspace(
        &self,
        context: &WorkspaceContext,
        workspace: &Workspace,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.create_workspace(context, workspace, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let workspace = workspace.clone();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.create_workspace(&context, &workspace, &audit).await
                })
            }
        }
    }

    fn update_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        name: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.update_workspace(context, workspace_id, name, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let workspace_id = workspace_id.to_owned();
                let name = name.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.update_workspace(&context, &workspace_id, &name, &audit).await
                })
            }
        }
    }

    fn delete_workspace(
        &self,
        context: &WorkspaceContext,
        workspace_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.delete_workspace(context, workspace_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let workspace_id = workspace_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.delete_workspace(&context, &workspace_id, &audit).await
                })
            }
        }
    }

    fn delete_group(
        &self,
        context: &WorkspaceContext,
        group_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.delete_group(context, group_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let group_id = group_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.delete_group(&context, &group_id, &audit).await
                })
            }
        }
    }

    fn delete_tag(&self, context: &WorkspaceContext, tag_id: &str, audit: &AuditEvent) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.delete_tag(context, tag_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let tag_id = tag_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.delete_tag(&context, &tag_id, &audit).await
                })
            }
        }
    }

    fn delete_service(
        &self,
        context: &WorkspaceContext,
        service_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.delete_service(context, service_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let service_id = service_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.delete_service(&context, &service_id, &audit).await
                })
            }
        }
    }

    fn delete_specification(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        audit: &AuditEvent,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.delete_specification(context, specification_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let specification_id = specification_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.delete_specification(&context, &specification_id, &audit).await
                })
            }
        }
    }

    fn set_workspace_member_role(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        role: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.set_workspace_member_role(context, actor_id, role, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let actor_id = actor_id.to_owned();
                let role = role.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.set_workspace_member_role(&context, &actor_id, &role, &audit).await
                })
            }
        }
    }

    fn revoke_workspace_member_with_audit(
        &self,
        context: &WorkspaceContext,
        actor_id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.revoke_workspace_member_with_audit(context, actor_id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let actor_id = actor_id.to_owned();
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.revoke_workspace_member_with_audit(&context, &actor_id, &audit).await
                })
            }
        }
    }

    #[cfg(test)]
    fn revoke_workspace_member(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        actor_id: &str,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.revoke_workspace_member(tenant_id, workspace_id, actor_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => Err(PersistenceError::new("use revoke_workspace_member_with_audit for PostgreSQL")),
        }
    }

    fn update_specification_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &Specification,
        updated: &Specification,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.update_specification_if_current_with_audit(context, expected, updated, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let expected = expected.clone();
                let updated = updated.clone();
                let audit = audit.cloned();
                run_postgres_future(store.clone(), move |store| async move {
                    store
                        .update_specification_if_current_with_audit(&context, &expected, &updated, audit.as_ref())
                        .await
                })
            }
        }
    }

    fn update_service_if_current_with_audit(
        &self,
        context: &WorkspaceContext,
        expected: &Service,
        updated: &Service,
        audit: Option<&AuditEvent>,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.update_service_if_current_with_audit(context, expected, updated, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let expected = expected.clone();
                let updated = updated.clone();
                let audit = audit.cloned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.update_service_if_current_with_audit(&context, &expected, &updated, audit.as_ref()).await
                })
            }
        }
    }

    fn specification_revision_count(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Result<i64, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.specification_revision_count(context, specification_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let specification_id = specification_id.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.specification_revision_count(&context, &specification_id).await
                })
            }
        }
    }

    fn specification_revisions(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SpecificationRevision>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.specification_revisions(context, specification_id, limit, offset),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let specification_id = specification_id.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.specification_revisions(&context, &specification_id, limit, offset).await
                })
            }
        }
    }

    fn specification_revision(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
        version: i64,
    ) -> Result<Option<SpecificationRevision>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.specification_revision(context, specification_id, version),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                let specification_id = specification_id.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.specification_revision(&context, &specification_id, version).await
                })
            }
        }
    }

    #[cfg(test)]
    fn audit_event_count(&self, context: &WorkspaceContext) -> Result<i64, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.audit_event_count(context),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let context = context.clone();
                run_postgres_future(store.clone(), move |store| async move { store.audit_event_count(&context).await })
            }
        }
    }

    fn create_workspace_backup(
        &self,
        context: &WorkspaceContext,
        root: &FsPath,
        prefix: &str,
        retention: usize,
    ) -> Result<BackupArtifact, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.create_workspace_backup(context, root, prefix, retention),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let root = root.to_path_buf();
                let prefix = prefix.to_owned();
                run_postgres_future(store.clone(), move |store| async move {
                    store.create_backup(root, &prefix, retention).await
                })
            }
        }
    }

    fn restore_workspace_from_path(
        &mut self,
        context: &WorkspaceContext,
        source: &FsPath,
    ) -> Result<(), PersistenceError> {
        match self {
            Self::Sqlite(store) => store.restore_workspace_from_path(context, source),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let source = source.to_path_buf();
                run_postgres_future(store.clone(), move |store| async move { store.restore_backup(source).await })
            }
        }
    }
}

#[cfg(feature = "postgres")]
fn run_postgres_future<T, F, Fut>(store: PostgresStore, operation: F) -> Result<T, PersistenceError>
where
    T: Send + 'static,
    F: FnOnce(PostgresStore) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, PersistenceError>> + Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| PersistenceError::new(error.to_string()))?;
        runtime.block_on(operation(store))
    })
    .join()
    .map_err(|error| PersistenceError::new(format!("PostgreSQL worker panicked: {error:?}")))?
}

impl ApiState {
    pub fn new(core: Core, store: SqliteStore) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ApiInner { store: StoreBackend::sqlite(store), seed_core: Some(core) })),
            backup_root: Arc::new(test_backup_root()),
            backup_storage: None,
            backup_retention: 10,
            metrics: Arc::new(ApiMetrics::storage_ready()),
            postgres_backend: false,
            public_origin: None,
        }
    }

    fn from_store(store: SqliteStore) -> Self {
        let backup_root = store
            .database_path()
            .and_then(|path| path.parent().map(|parent| parent.join("specx-backups")))
            .unwrap_or_else(test_backup_root);
        Self {
            inner: Arc::new(Mutex::new(ApiInner { store: StoreBackend::sqlite(store), seed_core: None })),
            backup_root: Arc::new(backup_root),
            backup_storage: None,
            backup_retention: 10,
            metrics: Arc::new(ApiMetrics::storage_ready()),
            postgres_backend: false,
            public_origin: None,
        }
    }

    fn from_store_with_backup_root(store: SqliteStore, backup_root: impl AsRef<FsPath>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ApiInner { store: StoreBackend::sqlite(store), seed_core: None })),
            backup_root: Arc::new(backup_root.as_ref().to_path_buf()),
            backup_storage: None,
            backup_retention: 10,
            metrics: Arc::new(ApiMetrics::storage_ready()),
            postgres_backend: false,
            public_origin: None,
        }
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, PersistenceError> {
        Ok(Self::from_store(SqliteStore::open(path)?))
    }

    #[cfg(feature = "postgres")]
    pub async fn open_postgres(url: &str, max_pool_size: usize) -> Result<Self, PersistenceError> {
        Self::open_postgres_with_backup_root(url, max_pool_size, test_backup_root()).await
    }

    #[cfg(feature = "postgres")]
    pub async fn open_postgres_with_backup_root(
        url: &str,
        max_pool_size: usize,
        backup_root: impl AsRef<FsPath>,
    ) -> Result<Self, PersistenceError> {
        let store = PostgresStore::connect(url, max_pool_size).await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ApiInner { store: StoreBackend::postgres(store), seed_core: None })),
            backup_root: Arc::new(backup_root.as_ref().to_path_buf()),
            backup_storage: None,
            backup_retention: 10,
            metrics: Arc::new(ApiMetrics::storage_ready()),
            postgres_backend: true,
            public_origin: None,
        })
    }

    /// Pin browser write requests to the origin published by a reverse proxy.
    /// This avoids trusting forwarded headers when the proxy's public authority
    /// differs from the authority seen by the API process.
    pub fn with_public_origin(mut self, origin: &str) -> Result<Self, PersistenceError> {
        self.public_origin = Some(parse_public_origin(origin.trim()).ok_or_else(|| {
            PersistenceError::new(
                "SPECX_PUBLIC_ORIGIN must be an absolute http(s) origin without a path, query, or credentials",
            )
        })?);
        Ok(self)
    }

    /// Whether this API state uses the shared PostgreSQL backend. Local
    /// SQLite snapshots are intentionally unavailable for that mode until the
    /// managed PostgreSQL backup job is configured by the host.
    pub fn uses_postgres(&self) -> bool {
        self.postgres_backend
    }

    pub fn has_shared_backup_storage(&self) -> bool {
        self.backup_storage.is_some()
    }

    /// Persist only the digest of a specX bearer session. The API facade owns
    /// this lifecycle so SQLite and PostgreSQL deployments share the same
    /// revocation semantics.
    pub async fn create_web_session(
        &self,
        token_hash: String,
        actor_id: String,
        expires_at: String,
        created_at: String,
    ) -> Result<(), PersistenceError> {
        let session = WebSession { token_hash, actor_id, expires_at, revoked_at: None, created_at };
        let inner = self.inner.lock().await;
        match &inner.store {
            StoreBackend::Sqlite(store) => store.create_web_session(&session),
            #[cfg(feature = "postgres")]
            StoreBackend::Postgres(store) => {
                run_postgres_future(store.clone(), move |store| async move { store.create_web_session(&session).await })
            }
        }
    }

    pub async fn web_session_is_active(&self, token_hash: String, now: String) -> Result<bool, PersistenceError> {
        let inner = self.inner.lock().await;
        match &inner.store {
            StoreBackend::Sqlite(store) => store.web_session_is_active(&token_hash, &now),
            #[cfg(feature = "postgres")]
            StoreBackend::Postgres(store) => run_postgres_future(store.clone(), move |store| async move {
                store.web_session_is_active(&token_hash, &now).await
            }),
        }
    }

    pub async fn web_session_actor_id(
        &self,
        token_hash: String,
        now: String,
    ) -> Result<Option<String>, PersistenceError> {
        let inner = self.inner.lock().await;
        match &inner.store {
            StoreBackend::Sqlite(store) => store.web_session_actor_id(&token_hash, &now),
            #[cfg(feature = "postgres")]
            StoreBackend::Postgres(store) => run_postgres_future(store.clone(), move |store| async move {
                store.web_session_actor_id(&token_hash, &now).await
            }),
        }
    }

    pub async fn revoke_web_session(&self, token_hash: String, revoked_at: String) -> Result<bool, PersistenceError> {
        let inner = self.inner.lock().await;
        match &inner.store {
            StoreBackend::Sqlite(store) => store.revoke_web_session(&token_hash, &revoked_at),
            #[cfg(feature = "postgres")]
            StoreBackend::Postgres(store) => run_postgres_future(store.clone(), move |store| async move {
                store.revoke_web_session(&token_hash, &revoked_at).await
            }),
        }
    }

    pub async fn revoke_other_web_sessions(
        &self,
        token_hash: String,
        revoked_at: String,
    ) -> Result<u64, PersistenceError> {
        let inner = self.inner.lock().await;
        match &inner.store {
            StoreBackend::Sqlite(store) => store.revoke_other_web_sessions(&token_hash, &revoked_at),
            #[cfg(feature = "postgres")]
            StoreBackend::Postgres(store) => run_postgres_future(store.clone(), move |store| async move {
                store.revoke_other_web_sessions(&token_hash, &revoked_at).await
            }),
        }
    }

    pub fn with_backup_retention(mut self, retention: usize) -> Result<Self, PersistenceError> {
        if retention == 0 {
            return Err(PersistenceError::new("specX backup retention must be at least one"));
        }
        self.backup_retention = retention;
        Ok(self)
    }

    pub async fn with_backup_storage_from_env(mut self) -> Result<Self, PersistenceError> {
        if let Some(storage) = BackupStorage::from_env()? {
            storage.validate().await?;
            self.backup_storage = Some(Arc::new(storage));
        }
        Ok(self)
    }

    #[cfg(feature = "postgres")]
    /// Rebuild the PostgreSQL pool after mounted TLS credentials have been
    /// rotated. The caller controls when the new certificate files become
    /// visible, so no secret material is copied into API state.
    pub async fn reload_postgres_credentials(&self, max_pool_size: usize) -> Result<(), PersistenceError> {
        let mut inner = self.inner.lock().await;
        match &mut inner.store {
            StoreBackend::Postgres(store) => store.reload_credentials(max_pool_size).await,
            StoreBackend::Sqlite(_) => Err(PersistenceError::new("PostgreSQL backend is not configured")),
        }
    }

    pub fn open_with_backup_root(
        path: impl AsRef<FsPath>,
        backup_root: impl AsRef<FsPath>,
    ) -> Result<Self, PersistenceError> {
        Ok(Self::from_store_with_backup_root(SqliteStore::open(path)?, backup_root))
    }

    pub async fn create_workspace_backup_snapshot(
        &self,
        context: &WorkspaceContext,
        retention: usize,
    ) -> Result<specx_core::persistence::BackupArtifact, PersistenceError> {
        let prefix = backup_prefix(context);
        let inner = self.inner.lock().await;
        let artifact = inner.store.create_workspace_backup(context, self.backup_root.as_ref(), &prefix, retention);
        drop(inner);
        let artifact = artifact.inspect_err(|_| {
            self.metrics.backup_creation_failures_total.fetch_add(1, Ordering::Relaxed);
        })?;
        if self.backup_storage.is_some() {
            let file_name =
                backup_file_name(&artifact.path).ok_or_else(|| PersistenceError::new("backup file name is invalid"))?;
            if let Err(error) = self
                .put_shared_backup(prefix, file_name, artifact.path.clone(), artifact.sha256.clone(), retention)
                .await
            {
                self.metrics.backup_creation_failures_total.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        }
        Ok(artifact)
    }

    #[allow(unused_variables)]
    async fn put_shared_backup(
        &self,
        scope_prefix: String,
        file_name: String,
        source: PathBuf,
        sha256: String,
        retention: usize,
    ) -> Result<(), PersistenceError> {
        let Some(storage) = self.backup_storage.clone() else {
            return Ok(());
        };
        #[cfg(not(feature = "s3-backup"))]
        {
            let _ = storage;
            Err(PersistenceError::new("shared S3 backup storage requires the s3-backup feature"))
        }
        #[cfg(feature = "s3-backup")]
        {
            #[cfg(feature = "postgres")]
            if self.postgres_backend {
                let store = {
                    let inner = self.inner.lock().await;
                    match &inner.store {
                        StoreBackend::Postgres(store) => store.clone(),
                        StoreBackend::Sqlite(_) => {
                            return Err(PersistenceError::new("PostgreSQL backup lock requested for SQLite backend"));
                        }
                    }
                };
                let lock_key = backup_retention_lock_key(&scope_prefix);
                return run_postgres_future(store, move |store| async move {
                    store
                        .with_backup_retention_lock(lock_key, move || async move {
                            storage.put_backup(&scope_prefix, &file_name, &source, &sha256, retention).await
                        })
                        .await
                });
            }
            storage.put_backup(&scope_prefix, &file_name, &source, &sha256, retention).await
        }
    }

    #[allow(unused_variables)]
    async fn list_shared_backups(
        &self,
        scope_prefix: String,
        extension: &'static str,
    ) -> Result<Vec<BackupObject>, PersistenceError> {
        let Some(storage) = self.backup_storage.clone() else {
            return Ok(Vec::new());
        };
        #[cfg(not(feature = "s3-backup"))]
        {
            let _ = storage;
            Err(PersistenceError::new("shared S3 backup storage requires the s3-backup feature"))
        }
        #[cfg(feature = "s3-backup")]
        {
            #[cfg(feature = "postgres")]
            if self.postgres_backend {
                let store = {
                    let inner = self.inner.lock().await;
                    match &inner.store {
                        StoreBackend::Postgres(store) => store.clone(),
                        StoreBackend::Sqlite(_) => {
                            return Err(PersistenceError::new("PostgreSQL backup lock requested for SQLite backend"));
                        }
                    }
                };
                let lock_key = backup_retention_lock_key(&scope_prefix);
                return run_postgres_future(store, move |store| async move {
                    store
                        .with_backup_retention_lock(lock_key, move || async move {
                            storage.list_backups(&scope_prefix, extension).await
                        })
                        .await
                });
            }
            storage.list_backups(&scope_prefix, extension).await
        }
    }

    #[allow(unused_variables)]
    async fn restore_shared_backup(
        &self,
        scope_prefix: String,
        file_name: String,
        destination: PathBuf,
    ) -> Result<(), PersistenceError> {
        let Some(storage) = self.backup_storage.clone() else {
            return Err(PersistenceError::new("shared backup storage is not configured"));
        };
        #[cfg(not(feature = "s3-backup"))]
        {
            let _ = storage;
            Err(PersistenceError::new("shared S3 backup storage requires the s3-backup feature"))
        }
        #[cfg(feature = "s3-backup")]
        {
            #[cfg(feature = "postgres")]
            if self.postgres_backend {
                let store = {
                    let inner = self.inner.lock().await;
                    match &inner.store {
                        StoreBackend::Postgres(store) => store.clone(),
                        StoreBackend::Sqlite(_) => {
                            return Err(PersistenceError::new("PostgreSQL backup lock requested for SQLite backend"));
                        }
                    }
                };
                let lock_key = backup_retention_lock_key(&scope_prefix);
                return run_postgres_future(store, move |store| async move {
                    store
                        .with_backup_retention_lock(lock_key, move || async move {
                            storage.restore_backup(&scope_prefix, &file_name, &destination).await
                        })
                        .await
                });
            }
            storage.restore_backup(&scope_prefix, &file_name, &destination).await
        }
    }

    pub async fn create_default_workspace_backup_snapshot(
        &self,
        retention: usize,
    ) -> Result<specx_core::persistence::BackupArtifact, PersistenceError> {
        self.create_workspace_backup_snapshot(
            &WorkspaceContext {
                tenant_id: "specx-web".to_string(),
                workspace_id: "default".to_string(),
                actor_id: "specx-web-system".to_string(),
            },
            retention,
        )
        .await
    }

    /// Delete expired pending invitations across the configured backend.
    /// The operation is host-triggered so deployments can choose the interval.
    pub async fn purge_expired_workspace_invitations(&self, now: &str) -> Result<u64, PersistenceError> {
        let inner = self.inner.lock().await;
        record_invitation_cleanup_result(&self.metrics, inner.store.purge_expired_workspace_invitations(now))
    }
}

#[cfg(all(feature = "postgres", feature = "s3-backup"))]
fn backup_retention_lock_key(scope_prefix: &str) -> (i32, i32) {
    let digest = Sha256::digest(scope_prefix.as_bytes());
    (
        i32::from_be_bytes(digest[..4].try_into().expect("SHA-256 prefix has four bytes")),
        i32::from_be_bytes(digest[4..8].try_into().expect("SHA-256 prefix has four bytes")),
    )
}

fn record_invitation_cleanup_result(
    metrics: &ApiMetrics,
    result: Result<u64, PersistenceError>,
) -> Result<u64, PersistenceError> {
    match result {
        Ok(purged) => {
            metrics.invitations_expired_total.fetch_add(purged, Ordering::Relaxed);
            Ok(purged)
        }
        Err(error) => {
            metrics.invitation_cleanup_failures_total.fetch_add(1, Ordering::Relaxed);
            Err(error)
        }
    }
}

async fn authorize_request(State(state): State<ApiState>, mut request: Request, next: Next) -> Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 100
                && value.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(generate_request_id);
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        request.headers_mut().insert("x-request-id", value);
    }
    let result = if matches!(request.uri().path(), "/health" | "/ready" | "/openapi.json") {
        next.run(request).await
    } else if state.metrics.ready.load(Ordering::Acquire) == 0 {
        ApiError::NotReady.into_response()
    } else if !write_origin_allowed(request.headers(), request.method(), state.public_origin.as_ref()) {
        ApiError::OriginRejected.into_response()
    } else if request.method() == axum::http::Method::POST && request.uri().path() == "/invitations/accept" {
        match invitation_actor_context(request.headers()) {
            Err(error) => error.into_response(),
            Ok(mut context) => {
                context.workspace_id = "__invitation_accept__".to_owned();
                match allow_request(&state, &context).await {
                    Ok(true) => next.run(request).await,
                    Ok(false) => ApiError::RateLimited.into_response(),
                    Err(error) => ApiError::Persistence(error).into_response(),
                }
            }
        }
    } else {
        match request_context(request.headers()) {
            Err(error) => error.into_response(),
            Ok(context) => {
                let is_workspace_directory =
                    request.method() == axum::http::Method::GET && request.uri().path() == "/workspaces";
                if is_workspace_directory {
                    let bootstrap_result =
                        if context.workspace_id == "default" && context.actor_id != "specx-web-anonymous" {
                            let inner = state.inner.lock().await;
                            inner.store.authorize_workspace(&context).map(|_| ())
                        } else {
                            Ok(())
                        };
                    let mut directory_context = context;
                    directory_context.workspace_id = "__workspace_directory__".into();
                    match bootstrap_result {
                        Err(error) => ApiError::Persistence(error).into_response(),
                        Ok(()) => match allow_request(&state, &directory_context).await {
                            Ok(false) => ApiError::RateLimited.into_response(),
                            Err(error) => ApiError::Persistence(error).into_response(),
                            Ok(true) => next.run(request).await,
                        },
                    }
                } else {
                    let authorization = {
                        let inner = state.inner.lock().await;
                        match inner.store.authorize_workspace(&context) {
                            Ok(true) => inner.store.workspace_member_role(&context, &context.actor_id),
                            Ok(false) => Ok(None),
                            Err(error) => Err(error),
                        }
                    };
                    match authorization {
                        Ok(None) => ApiError::Forbidden.into_response(),
                        Err(error) => ApiError::Persistence(error).into_response(),
                        Ok(Some(role)) => match allow_request(&state, &context).await {
                            Ok(false) => ApiError::RateLimited.into_response(),
                            Err(error) => ApiError::Persistence(error).into_response(),
                            Ok(true) if !is_read_method(request.method()) && !can_write(role.as_str()) => {
                                ApiError::Forbidden.into_response()
                            }
                            Ok(true) => next.run(request).await,
                        },
                    }
                }
            }
        }
    };
    apply_security_headers(result, &request_id)
}

fn is_read_method(method: &axum::http::Method) -> bool {
    matches!(*method, axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS)
}

fn write_origin_allowed(
    headers: &HeaderMap,
    method: &axum::http::Method,
    public_origin: Option<&PublicOrigin>,
) -> bool {
    if is_read_method(method) {
        return true;
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| matches!(value, "cross-site" | "cross-origin"))
    {
        return false;
    }
    let mut origins = headers.get_all(ORIGIN).iter();
    let Some(origin) = origins.next() else {
        return true;
    };
    if origins.next().is_some() {
        return false;
    }
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if let Some(public_origin) = public_origin {
        return parse_public_origin(origin).as_ref() == Some(public_origin);
    }
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    if parse_public_origin(origin).is_none() {
        return false;
    }
    origin
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|uri| uri.authority().cloned())
        .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host))
}

fn parse_public_origin(value: &str) -> Option<PublicOrigin> {
    let uri = value.parse::<axum::http::Uri>().ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return None;
    }
    if uri.path_and_query().is_some_and(|path| path.as_str() != "/") {
        return None;
    }
    let authority = uri.authority()?;
    if authority.as_str().contains('@') || authority.host().is_empty() {
        return None;
    }
    let port = authority.port_u16();
    if authority.port().is_some() && port.is_none() {
        return None;
    }
    let default_port = matches!((scheme.as_str(), port), ("http", Some(80)) | ("https", Some(443)));
    let host = authority.host().to_ascii_lowercase();
    let authority = match port.filter(|_| !default_port) {
        Some(port) => format!("{host}:{port}"),
        None => host,
    };
    Some(PublicOrigin { scheme, authority })
}

fn can_write(role: &str) -> bool {
    matches!(role, "owner" | "admin" | "editor")
}

fn can_manage_members(role: &str) -> bool {
    matches!(role, "owner" | "admin")
}

fn service_cursor_filter_digest(search: &str, tag_ids: &[Id], include_archived: bool) -> String {
    let payload =
        serde_json::to_vec(&(search, tag_ids, include_archived)).expect("service cursor filter is serializable");
    format!("{:x}", Sha256::digest(payload))
}

fn encode_service_cursor(search: &str, tag_ids: &[Id], include_archived: bool, offset: usize) -> String {
    format!("v1-{}-{:x}", service_cursor_filter_digest(search, tag_ids, include_archived), offset)
}

fn decode_service_cursor(cursor: &str, search: &str, tag_ids: &[Id], include_archived: bool) -> Option<usize> {
    let mut parts = cursor.split('-');
    if parts.next()? != "v1" || parts.next()? != service_cursor_filter_digest(search, tag_ids, include_archived) {
        return None;
    }
    let offset = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    usize::from_str_radix(offset, 16).ok()
}

fn generate_request_id() -> String {
    let sequence = REQUEST_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let time =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |duration| duration.as_micros());
    format!("sx-{time:x}-{sequence:x}")
}

async fn allow_request(state: &ApiState, context: &WorkspaceContext) -> Result<bool, PersistenceError> {
    const WINDOW_SECONDS: i64 = 60;
    const LIMIT: u32 = 300;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or_default();
    let inner = state.inner.lock().await;
    inner.store.consume_rate_limit(context, now, WINDOW_SECONDS, LIMIT)
}

fn apply_security_headers(mut response: Response, request_id: &str) -> Response {
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
    );
    if let Ok(value) = HeaderValue::from_str(request_id) {
        headers.insert("x-request-id", value);
    }
    response
}

fn load_core(inner: &mut ApiInner, context: &WorkspaceContext) -> Result<Core, ApiError> {
    match inner.seed_core.as_ref() {
        Some(core) => Ok(core.clone()),
        None => inner.store.load_core(context).map_err(ApiError::from),
    }
}

fn request_context(headers: &HeaderMap) -> Result<WorkspaceContext, ApiError> {
    let value = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
    };
    let tenant_id = value("x-specx-tenant-id").ok_or(ApiError::Unauthorized)?;
    let workspace_id = value("x-specx-workspace-id").ok_or(ApiError::Unauthorized)?;
    let actor_id = value("x-specx-actor-id").ok_or(ApiError::Unauthorized)?;
    Ok(WorkspaceContext { tenant_id, workspace_id, actor_id })
}

fn invitation_actor_context(headers: &HeaderMap) -> Result<WorkspaceContext, ApiError> {
    let value = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    Ok(WorkspaceContext {
        tenant_id: value("x-specx-tenant-id").ok_or(ApiError::Unauthorized)?,
        workspace_id: String::new(),
        actor_id: value("x-specx-actor-id").ok_or(ApiError::Unauthorized)?,
    })
}

fn idempotency_descriptor<T: Serialize>(
    headers: &HeaderMap,
    context: &WorkspaceContext,
    operation: &str,
    input: &T,
) -> Result<Option<(String, String)>, ApiError> {
    let Some(key) = headers.get("idempotency-key").and_then(|value| value.to_str().ok()) else {
        return Ok(None);
    };
    let key = key.trim();
    if key.is_empty()
        || key.len() > 200
        || !key.bytes().all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b'\\')
    {
        return Err(ApiError::InvalidQuery);
    }
    let payload = serde_json::to_vec(input).map_err(|_| ApiError::InvalidQuery)?;
    let fingerprint = format!("sha256:{:x}", Sha256::digest(payload));
    let scope_material =
        format!("{}\0{}\0{}\0{}\0{}", context.tenant_id, context.workspace_id, context.actor_id, operation, key);
    let scoped_key = format!("sx:{:x}", Sha256::digest(scope_material.as_bytes()));
    Ok(Some((scoped_key, fingerprint)))
}

fn check_idempotency(
    store: &StoreBackend,
    descriptor: Option<&(String, String)>,
) -> Result<Option<IdempotencyRecord>, ApiError> {
    let Some((key, fingerprint)) = descriptor else {
        return Ok(None);
    };
    match store.load_idempotency(key)? {
        None => Ok(None),
        Some(record) if record.fingerprint == *fingerprint => Ok(Some(record)),
        Some(_) => Err(ApiError::IdempotencyConflict),
    }
}

fn durable_idempotency<'a>(
    descriptor: Option<&'a (String, String)>,
    context: &'a WorkspaceContext,
    resource_type: &'a str,
    resource_id: &'a str,
) -> Option<(&'a str, &'a str, &'a str, &'a str, &'a str, &'a str)> {
    descriptor.map(|(key, fingerprint)| {
        (
            key.as_str(),
            context.tenant_id.as_str(),
            context.workspace_id.as_str(),
            fingerprint.as_str(),
            resource_type,
            resource_id,
        )
    })
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    NotFound,
    OriginRejected,
    RateLimited,
    NotReady,
    InvalidQuery,
    InvalidInput,
    LastOwnerRequired,
    IdempotencyConflict,
    PreconditionRequired,
    PreconditionFailed,
    Core(CoreError),
    Persistence(PersistenceError),
}

impl From<CoreError> for ApiError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}
impl From<PersistenceError> for ApiError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let kind = match self {
            Self::Unauthorized => ErrorKind::Unauthorized,
            Self::Forbidden => ErrorKind::Forbidden,
            Self::NotFound => ErrorKind::NotFound,
            Self::OriginRejected => ErrorKind::OriginRejected,
            Self::RateLimited => ErrorKind::RateLimited,
            Self::NotReady => ErrorKind::NotReady,
            Self::InvalidQuery => ErrorKind::InvalidQuery,
            Self::InvalidInput => ErrorKind::InvalidInput,
            Self::LastOwnerRequired => ErrorKind::LastOwnerRequired,
            Self::IdempotencyConflict => ErrorKind::IdempotencyConflict,
            Self::PreconditionRequired => ErrorKind::PreconditionRequired,
            Self::PreconditionFailed => ErrorKind::PreconditionFailed,
            Self::Core(error) => ErrorKind::from(error),
            Self::Persistence(error) => {
                tracing::error!(error = ?error, "storage operation failed");
                ErrorKind::Internal
            }
        };
        kind.into_response()
    }
}

#[cfg(test)]
mod tests;
