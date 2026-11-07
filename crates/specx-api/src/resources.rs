use super::*;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use axum::body::Bytes;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::Mac;
use specx_controller::{
    CommandGitProvider, ContentStore, GitCredential, RepositoryController, RepositoryReconcileRequest,
};

pub fn router<S>(state: ApiState) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/openapi.json", get(openapi_json))
        .route("/metrics", get(metrics))
        .route("/access", get(get_access))
        .route("/catalog", get(get_catalog))
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route("/workspaces/{workspace_id}", put(update_workspace).delete(delete_workspace))
        .route("/members", get(list_members))
        .route("/members/{member_actor_id}", put(set_member_role).delete(revoke_member))
        .route("/invitations", get(list_invitations).post(create_invitation))
        .route("/invitations/{invitation_id}", delete(revoke_invitation))
        .route("/invitations/accept", post(accept_invitation))
        .route("/groups", get(list_groups).post(create_group))
        .route("/groups/order", put(reorder_groups))
        .route("/groups/{group_id}", put(update_group).delete(delete_group))
        .route("/tags", get(list_tags).post(create_tag))
        .route("/tags/{tag_id}", put(update_tag).delete(delete_tag))
        .route("/services", get(list_services).post(create_service))
        .route("/services/{service_id}", put(update_service).patch(set_service_archived).delete(delete_service))
        .route(
            "/services/{service_id}/repository",
            get(get_service_repository).post(link_service_repository).delete(unlink_service_repository),
        )
        .route("/code-sources", get(list_code_sources).post(create_code_source))
        .route("/code-sources/{source_id}", put(update_code_source).delete(delete_code_source))
        .route("/services/{service_id}/code-bindings", get(list_service_code_bindings).post(create_code_binding))
        .route("/code-bindings/{binding_id}", put(update_code_binding).delete(delete_code_binding))
        .route("/specifications", get(list_specifications).post(create_specification))
        .route("/specifications/{specification_id}/revisions", get(list_specification_revisions))
        .route("/specifications/{specification_id}/revisions/{version}", get(get_specification_revision))
        .route("/specifications/{specification_id}/revisions/{version}/restore", post(restore_specification_revision))
        .route(
            "/specifications/{specification_id}/code-binding",
            get(get_specification_code_binding)
                .put(set_specification_code_binding)
                .delete(delete_specification_code_binding),
        )
        .route("/specifications/{specification_id}/sync-status", get(get_specification_sync_status))
        .route("/specifications/{specification_id}/sync", post(sync_specification))
        .route("/repositories/{repository_id}/files", get(discover_repository_files))
        .route("/repositories/{repository_id}/reconcile", post(enqueue_repository_reconcile))
        .route("/controller-tasks/{task_id}", get(get_controller_task))
        .route("/webhooks/repositories/{repository_id}", post(repository_webhook))
        .route("/secrets", get(list_secrets).post(create_secret))
        .route("/secrets/{secret_id}", put(update_secret).delete(delete_secret))
        .route("/integrations", get(list_integrations).post(create_integration))
        .route("/specifications/{specification_id}", put(update_specification).delete(delete_specification))
        .route("/backups", get(list_backups).post(create_backup))
        .route("/backups/{file_name}/restore", post(restore_backup))
        .fallback(|| async { ErrorKind::NotFound })
        .layer(middleware::from_fn_with_state(state.clone(), authorize_request))
        .layer(middleware::from_fn(errors::normalize_error_response))
        .with_state(state)
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
struct HealthResponse {
    status: &'static str,
    checked_at: String,
}

#[utoipa::path(get, path = "/health", responses((status = 200, description = "API health", body = HealthResponse)), tag = "system")]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok", checked_at: Utc::now().to_rfc3339() })
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct WorkspaceAccessResponse {
    tenant_id: Id,
    workspace_id: Id,
    actor_id: Id,
    role: String,
}

#[utoipa::path(
    get,
    path = "/access",
    responses(
        (status = 200, description = "Current actor access to the selected workspace", body = WorkspaceAccessResponse),
        (status = 403, description = "Actor is not an active workspace member", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn get_access(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<WorkspaceAccessResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    Ok(Json(WorkspaceAccessResponse {
        tenant_id: context.tenant_id,
        workspace_id: context.workspace_id,
        actor_id: context.actor_id,
        role,
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateWorkspaceRequest {
    pub name: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub name: String,
    pub slug: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    pub items: Vec<WorkspaceResponse>,
}

impl From<WorkspaceMembership> for WorkspaceResponse {
    fn from(membership: WorkspaceMembership) -> Self {
        Self {
            id: membership.workspace.id,
            tenant_id: membership.workspace.tenant_id,
            name: membership.workspace.name,
            slug: membership.workspace.slug,
            created_at: membership.workspace.created_at,
            role: membership.role,
        }
    }
}

#[utoipa::path(
    get,
    path = "/workspaces",
    responses(
        (status = 200, description = "Workspaces accessible to the authenticated actor", body = WorkspaceListResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "workspaces"
)]
async fn list_workspaces(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<WorkspaceListResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let items =
        inner.store.workspaces_for_actor(&context.tenant_id, &context.actor_id)?.into_iter().map(Into::into).collect();
    Ok(Json(WorkspaceListResponse { items }))
}

#[utoipa::path(
    post,
    path = "/workspaces",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Workspace created with the actor as owner", body = WorkspaceResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Workspace creation requires owner or admin role", body = ErrorResponse),
        (status = 422, description = "Invalid workspace name", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "workspaces"
)]
async fn create_workspace(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    let context = request_context(&headers)?;
    let name = input.name.trim();
    if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        return Err(ApiError::InvalidInput);
    }
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    let request_id = headers.get("x-request-id").and_then(|value| value.to_str().ok()).unwrap_or("request");
    let digest =
        format!("{:x}", Sha256::digest(format!("{}\0{}\0{}", context.tenant_id, context.actor_id, request_id)));
    let id = format!("workspace-{digest}");
    let slug = workspace_slug(name, &digest);
    let created_at = Utc::now().to_rfc3339();
    let workspace = Workspace {
        id: id.clone(),
        tenant_id: context.tenant_id.clone(),
        name: name.to_owned(),
        slug,
        created_at: created_at.clone(),
    };
    let mut audit = audit_event(&context, &headers, "create", "workspace", &id);
    audit.workspace_id = id.clone();
    if !inner.store.create_workspace(&context, &workspace, &audit)? {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(WorkspaceResponse {
        id: workspace.id,
        tenant_id: workspace.tenant_id,
        name: workspace.name,
        slug: workspace.slug,
        role: "owner".into(),
        created_at: workspace.created_at,
    }))
}

#[utoipa::path(
    put,
    path = "/workspaces/{workspace_id}",
    params(("workspace_id" = String, Path, description = "Workspace identifier")),
    request_body = UpdateWorkspaceRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Workspace updated", body = WorkspaceResponse),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse)
    ),
    tag = "workspaces"
)]
async fn update_workspace(
    State(state): State<ApiState>,
    Path(workspace_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    let context = request_context(&headers)?;
    let name = input.name.trim();
    if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        return Err(ApiError::InvalidInput);
    }
    let inner = state.inner.lock().await;
    let membership = inner
        .store
        .workspaces_for_actor(&context.tenant_id, &context.actor_id)?
        .into_iter()
        .find(|membership| membership.workspace.id == workspace_id)
        .ok_or(ApiError::Forbidden)?;
    let audit = audit_event(&context, &headers, "update", "workspace", &workspace_id);
    if !inner.store.update_workspace(&context, &workspace_id, name, &audit)? {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(WorkspaceResponse {
        id: membership.workspace.id,
        tenant_id: membership.workspace.tenant_id,
        name: name.to_owned(),
        slug: membership.workspace.slug,
        role: membership.role,
        created_at: membership.workspace.created_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{workspace_id}",
    params(("workspace_id" = String, Path, description = "Workspace identifier")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Workspace and its scoped facts deleted"),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse)
    ),
    tag = "workspaces"
)]
async fn delete_workspace(
    State(state): State<ApiState>,
    Path(workspace_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let audit = audit_event(&context, &headers, "delete", "workspace", &workspace_id);
    if !inner.store.delete_workspace(&context, &workspace_id, &audit)? {
        return Err(ApiError::Forbidden);
    }
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

fn workspace_slug(name: &str, digest: &str) -> String {
    let mut base = String::new();
    for character in name.to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            base.push(character);
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    let base = base.trim_matches('-');
    let base = if base.is_empty() { "workspace" } else { base };
    format!("{base}-{}", &digest[..8])
}

#[utoipa::path(
    get,
    path = "/metrics",
    responses((status = 200, description = "Prometheus metrics")),
    tag = "system"
)]
async fn metrics(State(state): State<ApiState>) -> Response {
    let ready = state.inner.lock().await.store.ready().is_ok_and(|is_ready| is_ready);
    state.metrics.ready.store(u64::from(ready), Ordering::Relaxed);
    let body = format!(
        "# TYPE specx_api_requests_total counter\nspecx_api_requests_total {}\n# TYPE specx_backup_created_total counter\nspecx_backup_created_total {}\n# TYPE specx_backup_creation_failures_total counter\nspecx_backup_creation_failures_total {}\n# TYPE specx_backup_restored_total counter\nspecx_backup_restored_total {}\n# TYPE specx_backup_restore_failures_total counter\nspecx_backup_restore_failures_total {}\n# TYPE specx_invitations_expired_total counter\nspecx_invitations_expired_total {}\n# TYPE specx_invitation_cleanup_failures_total counter\nspecx_invitation_cleanup_failures_total {}\n# TYPE specx_api_ready gauge\nspecx_api_ready {}\n",
        state.metrics.requests_total.load(Ordering::Relaxed),
        state.metrics.backups_created_total.load(Ordering::Relaxed),
        state.metrics.backup_creation_failures_total.load(Ordering::Relaxed),
        state.metrics.backups_restored_total.load(Ordering::Relaxed),
        state.metrics.backup_restore_failures_total.load(Ordering::Relaxed),
        state.metrics.invitations_expired_total.load(Ordering::Relaxed),
        state.metrics.invitation_cleanup_failures_total.load(Ordering::Relaxed),
        state.metrics.ready.load(Ordering::Relaxed),
    );
    ([(CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response()
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReadyResponse {
    status: &'static str,
    schema_version: i64,
}

#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Storage migrations are ready", body = ReadyResponse),
        (status = 503, description = "Storage is not ready", body = ErrorResponse)
    ),
    tag = "system"
)]
async fn ready(State(state): State<ApiState>) -> Result<Json<ReadyResponse>, ApiError> {
    let inner = state.inner.lock().await;
    if !inner.store.ready()? {
        state.metrics.ready.store(0, Ordering::Relaxed);
        return Err(ApiError::NotReady);
    }
    state.metrics.ready.store(1, Ordering::Relaxed);
    Ok(Json(ReadyResponse { status: "ready", schema_version: inner.store.schema_version() }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct BackupResponse {
    file_name: String,
    bytes: u64,
    sha256: Option<String>,
}

pub(super) fn backup_file_name(path: &FsPath) -> Option<String> {
    path.file_name().and_then(|name| name.to_str()).map(ToOwned::to_owned)
}

pub(super) fn backup_prefix(context: &WorkspaceContext) -> String {
    let digest = Sha256::digest(format!("{}\0{}", context.tenant_id, context.workspace_id).as_bytes());
    let suffix = digest[..8].iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    format!("specx-{suffix}")
}

fn safe_backup_path(root: &FsPath, prefix: &str, file_name: &str, extension: &str) -> Result<PathBuf, ApiError> {
    if !valid_backup_file_name(file_name, prefix, extension) {
        return Err(ApiError::InvalidQuery);
    }
    Ok(root.join(file_name))
}

fn valid_backup_file_name(file_name: &str, prefix: &str, extension: &str) -> bool {
    !file_name.is_empty()
        && file_name.len() <= 160
        && file_name.starts_with(prefix)
        && file_name.ends_with(extension)
        && file_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn backup_extension(state: &ApiState) -> &'static str {
    if state.uses_postgres() { ".dump" } else { ".sqlite" }
}

async fn require_backup_manager(state: &ApiState, headers: &HeaderMap) -> Result<WorkspaceContext, ApiError> {
    let context = request_context(headers)?;
    if state.uses_postgres() && (context.tenant_id != "specx-web" || context.workspace_id != "default") {
        return Err(ApiError::Forbidden);
    }
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    Ok(context)
}

#[utoipa::path(
    get,
    path = "/backups",
    responses((status = 200, description = "Available specX storage snapshots", body = [BackupResponse]), (status = 403, body = ErrorResponse)),
    tag = "backups"
)]
async fn list_backups(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<BackupResponse>>, ApiError> {
    let context = require_backup_manager(&state, &headers).await?;
    let prefix = backup_prefix(&context);
    let extension = backup_extension(&state);
    if state.backup_storage.is_some() {
        let backups = state
            .list_shared_backups(prefix, extension)
            .await?
            .into_iter()
            .map(|backup| BackupResponse { file_name: backup.file_name, bytes: backup.bytes, sha256: None })
            .collect();
        return Ok(Json(backups));
    }
    let root = state.backup_root.as_ref();
    let mut backups = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Json(backups)),
        Err(error) => return Err(ApiError::Persistence(PersistenceError::from(error))),
    };
    for entry in entries {
        let path = entry.map_err(PersistenceError::from)?.path();
        let Some(file_name) = backup_file_name(&path) else { continue };
        if !file_name.starts_with(&prefix) || !file_name.ends_with(extension) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(PersistenceError::from)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        backups.push(BackupResponse { file_name, bytes: metadata.len(), sha256: None });
    }
    backups.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(Json(backups))
}

#[utoipa::path(
    post,
    path = "/backups",
    responses((status = 200, description = "Snapshot created", body = BackupResponse), (status = 403, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "backups"
)]
async fn create_backup(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<BackupResponse>, ApiError> {
    let context = require_backup_manager(&state, &headers).await?;
    let artifact = state.create_workspace_backup_snapshot(&context, state.backup_retention).await?;
    state.metrics.backups_created_total.fetch_add(1, Ordering::Relaxed);
    Ok(Json(BackupResponse {
        file_name: backup_file_name(&artifact.path).ok_or(ApiError::Persistence(PersistenceError::from(
            std::io::Error::new(std::io::ErrorKind::InvalidData, "backup file name is invalid"),
        )))?,
        bytes: artifact.bytes,
        sha256: Some(artifact.sha256),
    }))
}

#[utoipa::path(
    post,
    path = "/backups/{file_name}/restore",
    params(("file_name" = String, Path, description = "Server-generated SQLite snapshot name")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),(status = 200, description = "Snapshot restored"), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse)),
    tag = "backups"
)]
async fn restore_backup(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(file_name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let context = require_backup_manager(&state, &headers).await?;
    let prefix = backup_prefix(&context);
    let extension = backup_extension(&state);
    let (source, remove_source) = if state.backup_storage.is_some() {
        if !valid_backup_file_name(&file_name, &prefix, extension) {
            return Err(ApiError::InvalidQuery);
        }
        std::fs::create_dir_all(state.backup_root.as_ref()).map_err(PersistenceError::from)?;
        let source = state.backup_root.join(format!(".restore-{}{}", uuid::Uuid::new_v4(), extension));
        if let Err(error) = state.restore_shared_backup(prefix.clone(), file_name.clone(), source.clone()).await {
            state.metrics.backup_restore_failures_total.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::Persistence(error));
        }
        (source, true)
    } else {
        let source = safe_backup_path(state.backup_root.as_ref(), &prefix, &file_name, extension)?;
        let metadata = std::fs::symlink_metadata(&source).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ApiError::NotFound
            } else {
                ApiError::Persistence(PersistenceError::from(error))
            }
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ApiError::NotFound);
        }
        (source, false)
    };
    let mut inner = state.inner.lock().await;
    let restore_result = inner.store.restore_workspace_from_path(&context, &source);
    drop(inner);
    if remove_source {
        let _ = std::fs::remove_file(&source);
    }
    if let Err(error) = restore_result {
        state.metrics.backup_restore_failures_total.fetch_add(1, Ordering::Relaxed);
        return Err(ApiError::Persistence(error));
    }
    state.metrics.backups_restored_total.fetch_add(1, Ordering::Relaxed);
    Ok(Json(serde_json::json!({"restored": true, "fileName": file_name})))
}

fn audit_event(
    context: &WorkspaceContext,
    headers: &HeaderMap,
    action: &str,
    resource_type: &str,
    resource_id: &str,
) -> AuditEvent {
    let request_id = headers.get("x-request-id").and_then(|value| value.to_str().ok()).unwrap_or_default();
    AuditEvent {
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        actor_id: context.actor_id.clone(),
        request_id: request_id.to_owned(),
        action: action.to_owned(),
        resource_type: resource_type.to_owned(),
        resource_id: resource_id.to_owned(),
        created_at: Utc::now().to_rfc3339(),
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceGroupResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub parent_id: Option<Id>,
    pub name: String,
    pub position: i64,
}

impl From<ServiceGroup> for ServiceGroupResponse {
    fn from(group: ServiceGroup) -> Self {
        Self {
            id: group.id,
            tenant_id: group.tenant_id,
            workspace_id: group.workspace_id,
            parent_id: group.parent_id,
            name: group.name,
            position: group.position,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub name: String,
    pub color: String,
}

impl From<Tag> for TagResponse {
    fn from(tag: Tag) -> Self {
        Self { id: tag.id, tenant_id: tag.tenant_id, workspace_id: tag.workspace_id, name: tag.name, color: tag.color }
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub group_id: Option<Id>,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub tag_ids: Vec<Id>,
    pub updated_at: String,
    pub archived: bool,
    pub etag: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeSourceResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    #[schema(value_type = String)]
    pub source_type: CodeSourceType,
    pub provider: String,
    pub display_name: String,
    pub canonical_key: String,
    pub root_locator: String,
    #[schema(value_type = String)]
    pub execution_scope: CodeExecutionScope,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
    pub etag: String,
}

impl From<CodeSource> for CodeSourceResponse {
    fn from(source: CodeSource) -> Self {
        let mut response = Self {
            id: source.id,
            tenant_id: source.tenant_id,
            workspace_id: source.workspace_id,
            source_type: source.source_type,
            provider: source.provider,
            display_name: source.display_name,
            canonical_key: source.canonical_key,
            root_locator: source.root_locator,
            execution_scope: source.execution_scope,
            created_at: source.created_at,
            updated_at: source.updated_at,
            archived: source.archived,
            etag: String::new(),
        };
        response.etag = code_source_etag(&response);
        response
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCodeBindingResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub service_id: Id,
    pub source_id: Id,
    #[schema(value_type = String)]
    pub role: CodeBindingRole,
    #[schema(value_type = String)]
    pub entity_kind: CodeEntityKind,
    pub path: String,
    #[schema(value_type = String)]
    pub revision_kind: CodeRevisionKind,
    pub revision: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub etag: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummaryResponse {
    pub id: Id,
    pub provider: String,
    pub display_name: String,
    pub canonical_url: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRepositoryResponse {
    pub id: Id,
    pub repository: RepositorySummaryResponse,
    pub created_at: String,
    pub updated_at: String,
    pub etag: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationCodeBindingResponse {
    pub id: Id,
    pub specification_id: Id,
    pub repository: RepositorySummaryResponse,
    pub path: String,
    #[schema(value_type = String)]
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub etag: String,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetSpecificationCodeBindingRequest {
    #[schema(value_type = String)]
    pub ref_kind: SpecificationRefKind,
    #[serde(default)]
    pub path: String,
    pub ref_name: String,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkServiceRepositoryRequest {
    pub repository_url: String,
}

impl From<ServiceCodeBinding> for ServiceCodeBindingResponse {
    fn from(binding: ServiceCodeBinding) -> Self {
        let mut response = Self {
            id: binding.id,
            tenant_id: binding.tenant_id,
            workspace_id: binding.workspace_id,
            service_id: binding.service_id,
            source_id: binding.source_id,
            role: binding.role,
            entity_kind: binding.entity_kind,
            path: binding.path,
            revision_kind: binding.revision_kind,
            revision: binding.revision,
            created_at: binding.created_at,
            updated_at: binding.updated_at,
            etag: String::new(),
        };
        response.etag = code_binding_etag(&response);
        response
    }
}

fn service_repository_response(
    core: &Core,
    context: &WorkspaceContext,
    service_id: &str,
) -> Result<ServiceRepositoryResponse, CoreError> {
    let relation = core.service_repository(context, service_id).ok_or(CoreError::NotFound("service_repository"))?;
    let repository: &Repository =
        core.repository(context, &relation.repository_id).ok_or(CoreError::NotFound("repository"))?;
    let mut response = ServiceRepositoryResponse {
        id: relation.id.clone(),
        repository: RepositorySummaryResponse {
            id: repository.id.clone(),
            provider: repository.provider.clone(),
            display_name: repository.display_name.clone(),
            canonical_url: repository.canonical_url.clone(),
        },
        created_at: relation.created_at.clone(),
        updated_at: relation.updated_at.clone(),
        etag: String::new(),
    };
    response.etag = service_repository_etag(&response);
    Ok(response)
}

fn specification_code_binding_response(
    core: &Core,
    context: &WorkspaceContext,
    binding: &SpecificationCodeBinding,
) -> Result<SpecificationCodeBindingResponse, CoreError> {
    let specification = core
        .list_specifications(context)
        .into_iter()
        .find(|specification| specification.id == binding.specification_id)
        .ok_or(CoreError::NotFound("specification"))?;
    let relation = core
        .service_repository(context, &specification.service_id)
        .filter(|relation| relation.id == binding.service_repository_id)
        .ok_or(CoreError::NotFound("service_repository"))?;
    let repository = core.repository(context, &relation.repository_id).ok_or(CoreError::NotFound("repository"))?;
    let mut response = SpecificationCodeBindingResponse {
        id: binding.id.clone(),
        specification_id: binding.specification_id.clone(),
        repository: RepositorySummaryResponse {
            id: repository.id.clone(),
            provider: repository.provider.clone(),
            display_name: repository.display_name.clone(),
            canonical_url: repository.canonical_url.clone(),
        },
        path: binding.path.clone(),
        ref_kind: binding.ref_kind,
        ref_name: binding.ref_name.clone(),
        created_at: binding.created_at.clone(),
        updated_at: binding.updated_at.clone(),
        etag: String::new(),
    };
    response.etag = specification_code_binding_etag(&response);
    Ok(response)
}

impl From<Service> for ServiceResponse {
    fn from(service: Service) -> Self {
        let mut response = Self {
            id: service.id,
            tenant_id: service.tenant_id,
            workspace_id: service.workspace_id,
            group_id: service.group_id,
            name: service.name,
            slug: service.slug,
            description: service.description,
            tag_ids: service.tag_ids,
            updated_at: service.updated_at,
            archived: service.archived,
            etag: String::new(),
        };
        response.tag_ids.sort();
        response.etag = service_etag(&response);
        response
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpecificationKindResponse {
    Markdown,
}

impl From<SpecificationKind> for SpecificationKindResponse {
    fn from(kind: SpecificationKind) -> Self {
        match kind {
            SpecificationKind::Markdown => Self::Markdown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationResponse {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub service_id: Id,
    pub title: String,
    pub kind: SpecificationKindResponse,
    pub content: String,
    pub version: i64,
    pub content_hash: String,
    pub updated_at: String,
    pub etag: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationSyncResponse {
    pub specification: SpecificationResponse,
    pub snapshot_id: Id,
    pub manifest_hash: String,
    pub resolved_commit: String,
    pub content_hash: String,
    pub changed: bool,
    pub synced_at: String,
    pub provider_version: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryDiscoveryQuery {
    #[schema(value_type = String)]
    ref_kind: SpecificationRefKind,
    ref_name: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct RepositoryFileResponse {
    path: String,
    byte_length: u64,
    content_hash: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct RepositoryDiscoveryResponse {
    snapshot_id: Id,
    resolved_commit: String,
    manifest_hash: String,
    provider_version: String,
    files: Vec<RepositoryFileResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct ControllerTaskResponse {
    id: Id,
    controller_id: String,
    resource_kind: String,
    resource_id: String,
    status: String,
    attempts: i64,
    next_run_at: String,
    lease_until: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<ControllerTask> for ControllerTaskResponse {
    fn from(task: ControllerTask) -> Self {
        Self {
            id: task.id,
            controller_id: task.controller_id,
            resource_kind: task.resource_kind,
            resource_id: task.resource_id,
            status: task.status,
            attempts: task.attempts,
            next_run_at: task.next_run_at,
            lease_until: task.lease_until,
            last_error: task.last_error,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryWebhookRequest {
    tenant_id: String,
    workspace_id: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSecretRequest {
    id: Id,
    name: String,
    value: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct SecretResponse {
    id: Id,
    name: String,
    key_version: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct SecretPageResponse {
    items: Vec<SecretResponse>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateSecretRequest {
    value: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateIntegrationRequest {
    id: Id,
    provider: String,
    display_name: String,
    secret_id: Option<Id>,
    permissions: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct IntegrationResponse {
    id: Id,
    provider: String,
    display_name: String,
    secret_id: Option<Id>,
    permissions: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct IntegrationPageResponse {
    items: Vec<IntegrationResponse>,
}

#[utoipa::path(
    get,
    path = "/specifications/{specification_id}/sync-status",
    params(("specification_id" = String, Path)),
    responses(
        (status = 200, description = "Latest persisted repository synchronization evidence", body = SpecificationSyncResponse),
        (status = 404, body = ErrorResponse)
    ),
    tag = "code"
)]
async fn get_specification_sync_status(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Json<SpecificationSyncResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let specification = core
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let binding = core
        .specification_code_binding(&context, &specification_id)
        .filter(|binding| !binding.path.is_empty())
        .ok_or(CoreError::NotFound("specification_code_binding"))?;
    let relation = core
        .service_repository(&context, &specification.service_id)
        .ok_or(CoreError::NotFound("service_repository"))?;
    let snapshot = core
        .list_repository_snapshots(&context, &relation.repository_id)
        .into_iter()
        .find(|snapshot| snapshot.source_ref == binding.ref_name)
        .ok_or(CoreError::NotFound("repository_snapshot"))?;
    let content_hash = specification.content_hash.clone();
    Ok(Json(SpecificationSyncResponse {
        specification: specification.into(),
        snapshot_id: snapshot.id.clone(),
        manifest_hash: snapshot.manifest_hash.clone(),
        resolved_commit: snapshot.resolved_commit.clone(),
        content_hash,
        changed: false,
        synced_at: snapshot.fetched_at.clone(),
        provider_version: snapshot.provider_version.clone(),
    }))
}

#[utoipa::path(
    get,
    path = "/repositories/{repository_id}/files",
    params(
        ("repository_id" = String, Path),
        ("refKind" = String, Query),
        ("refName" = String, Query)
    ),
    responses((status = 200, body = RepositoryDiscoveryResponse), (status = 404, body = ErrorResponse), (status = 502, body = ErrorResponse)),
    tag = "code"
)]
async fn discover_repository_files(
    State(state): State<ApiState>,
    Path(repository_id): Path<Id>,
    Query(query): Query<RepositoryDiscoveryQuery>,
    headers: HeaderMap,
) -> Result<Json<RepositoryDiscoveryResponse>, ApiError> {
    let context = request_context(&headers)?;
    let (repository_url, credential) = {
        let mut inner = state.inner.lock().await;
        let repository = load_core(&mut inner, &context)?
            .repository(&context, &repository_id)
            .ok_or(CoreError::NotFound("repository"))?
            .clone();
        let credential = repository_credential(&inner, &context, &repository.provider)?;
        (repository.canonical_url, credential)
    };
    let fetched = fetch_repository_snapshot(repository_url, query.ref_kind, query.ref_name.clone(), credential).await?;
    let fetched_at = Utc::now().to_rfc3339();
    let snapshot =
        persist_repository_snapshot(&state, &context, &repository_id, &query.ref_name, &fetched, &fetched_at).await?;
    let files = fetched
        .files
        .into_iter()
        .filter(|file| {
            let path = file.path.to_ascii_lowercase();
            path.ends_with(".md") || path.ends_with(".markdown")
        })
        .map(|file| RepositoryFileResponse {
            path: file.path,
            byte_length: file.content.len() as u64,
            content_hash: file.content_hash,
        })
        .collect();
    Ok(Json(RepositoryDiscoveryResponse {
        snapshot_id: snapshot.id,
        resolved_commit: fetched.resolved_commit,
        manifest_hash: fetched.manifest_hash,
        provider_version: fetched.provider_version,
        files,
    }))
}

#[utoipa::path(
    post,
    path = "/repositories/{repository_id}/reconcile",
    params(("repository_id" = String, Path)),
    responses((status = 202, body = ControllerTaskResponse), (status = 404, body = ErrorResponse)),
    tag = "code"
)]
async fn enqueue_repository_reconcile(
    State(state): State<ApiState>,
    Path(repository_id): Path<Id>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<ControllerTaskResponse>), ApiError> {
    let context = request_context(&headers)?;
    {
        let mut inner = state.inner.lock().await;
        load_core(&mut inner, &context)?
            .repository(&context, &repository_id)
            .ok_or(CoreError::NotFound("repository"))?;
    }
    let task_id = state.enqueue_repository_task(&context, &repository_id).await?;
    let task = state.controller_task(&task_id).await?.ok_or(ApiError::NotFound)?;
    Ok((StatusCode::ACCEPTED, Json(task.into())))
}

#[utoipa::path(
    get,
    path = "/controller-tasks/{task_id}",
    params(("task_id" = String, Path)),
    responses((status = 200, body = ControllerTaskResponse), (status = 404, body = ErrorResponse)),
    tag = "controllers"
)]
async fn get_controller_task(
    State(state): State<ApiState>,
    Path(task_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Json<ControllerTaskResponse>, ApiError> {
    let context = request_context(&headers)?;
    let task = state.controller_task(&task_id).await?.ok_or(ApiError::NotFound)?;
    if task.tenant_id != context.tenant_id || task.workspace_id != context.workspace_id {
        return Err(ApiError::NotFound);
    }
    Ok(Json(task.into()))
}

#[utoipa::path(
    post,
    path = "/webhooks/repositories/{repository_id}",
    params(("repository_id" = String, Path), ("X-SpecX-Webhook-Signature" = String, Header)),
    request_body = RepositoryWebhookRequest,
    responses((status = 202, body = ControllerTaskResponse), (status = 401, body = ErrorResponse), (status = 404, body = ErrorResponse)),
    tag = "controllers"
)]
async fn repository_webhook(
    State(state): State<ApiState>,
    Path(repository_id): Path<Id>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<ControllerTaskResponse>), ApiError> {
    let signature = headers.get("x-specx-webhook-signature").and_then(|value| value.to_str().ok()).unwrap_or("");
    if !verify_webhook_signature(state.webhook_secret(), signature, &body) {
        return Err(ApiError::Unauthorized);
    }
    let input: RepositoryWebhookRequest = serde_json::from_slice(&body).map_err(|_| ApiError::InvalidInput)?;
    let context = WorkspaceContext {
        tenant_id: input.tenant_id,
        workspace_id: input.workspace_id,
        actor_id: "specx-webhook".into(),
    };
    {
        let mut inner = state.inner.lock().await;
        load_core(&mut inner, &context)?
            .repository(&context, &repository_id)
            .ok_or(CoreError::NotFound("repository"))?;
    }
    let task_id = state.enqueue_repository_task(&context, &repository_id).await?;
    let task = state.controller_task(&task_id).await?.ok_or(ApiError::NotFound)?;
    Ok((StatusCode::ACCEPTED, Json(task.into())))
}

#[utoipa::path(get, path = "/secrets", responses((status = 200, body = SecretPageResponse)), tag = "integrations")]
async fn list_secrets(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<SecretPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let items =
        inner.store.list_secrets(&context.tenant_id, &context.workspace_id)?.into_iter().map(secret_response).collect();
    Ok(Json(SecretPageResponse { items }))
}

#[utoipa::path(post, path = "/secrets", request_body = CreateSecretRequest, responses((status = 200, body = SecretResponse), (status = 422, body = ErrorResponse), (status = 503, body = ErrorResponse)), tag = "integrations")]
async fn create_secret(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateSecretRequest>,
) -> Result<Json<SecretResponse>, ApiError> {
    let context = request_context(&headers)?;
    validate_secret_input(&input.name, &input.value)?;
    let now = Utc::now().to_rfc3339();
    let encrypted_value = encrypt_secret(&input.value)?;
    let secret = SecretRecord {
        id: input.id,
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        name: input.name.trim().to_owned(),
        encrypted_value,
        key_version: 1,
        created_at: now.clone(),
        updated_at: now,
    };
    let inner = state.inner.lock().await;
    inner.store.upsert_secret(&secret)?;
    Ok(Json(secret_response(secret)))
}

#[utoipa::path(put, path = "/secrets/{secret_id}", params(("secret_id" = String, Path)), request_body = UpdateSecretRequest, responses((status = 200, body = SecretResponse), (status = 404, body = ErrorResponse), (status = 422, body = ErrorResponse), (status = 503, body = ErrorResponse)), tag = "integrations")]
async fn update_secret(
    State(state): State<ApiState>,
    Path(secret_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateSecretRequest>,
) -> Result<Json<SecretResponse>, ApiError> {
    let context = request_context(&headers)?;
    validate_secret_input("valid", &input.value)?;
    let encrypted_value = encrypt_secret(&input.value)?;
    let inner = state.inner.lock().await;
    let existing =
        inner.store.secret_by_id(&context.tenant_id, &context.workspace_id, &secret_id)?.ok_or(ApiError::NotFound)?;
    let updated = SecretRecord { encrypted_value, updated_at: Utc::now().to_rfc3339(), ..existing };
    inner.store.upsert_secret(&updated)?;
    Ok(Json(secret_response(updated)))
}

#[utoipa::path(delete, path = "/secrets/{secret_id}", params(("secret_id" = String, Path)), responses((status = 204), (status = 404, body = ErrorResponse)), tag = "integrations")]
async fn delete_secret(
    State(state): State<ApiState>,
    Path(secret_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    if !inner.store.delete_secret(&context.tenant_id, &context.workspace_id, &secret_id)? {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/integrations", responses((status = 200, body = IntegrationPageResponse)), tag = "integrations")]
async fn list_integrations(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<IntegrationPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let items = inner
        .store
        .list_integrations(&context.tenant_id, &context.workspace_id)?
        .into_iter()
        .map(integration_response)
        .collect();
    Ok(Json(IntegrationPageResponse { items }))
}

#[utoipa::path(post, path = "/integrations", request_body = CreateIntegrationRequest, responses((status = 200, body = IntegrationResponse), (status = 404, body = ErrorResponse), (status = 422, body = ErrorResponse)), tag = "integrations")]
async fn create_integration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateIntegrationRequest>,
) -> Result<Json<IntegrationResponse>, ApiError> {
    let context = request_context(&headers)?;
    if input.provider.trim().is_empty() || input.display_name.trim().is_empty() || input.permissions.len() > 32 {
        return Err(ApiError::InvalidInput);
    }
    let inner = state.inner.lock().await;
    if let Some(secret_id) = &input.secret_id {
        inner.store.secret_by_id(&context.tenant_id, &context.workspace_id, secret_id)?.ok_or(ApiError::NotFound)?;
    }
    let now = Utc::now().to_rfc3339();
    let integration = IntegrationRecord {
        id: input.id,
        tenant_id: context.tenant_id,
        workspace_id: context.workspace_id,
        provider: input.provider.trim().to_ascii_lowercase(),
        display_name: input.display_name.trim().to_owned(),
        secret_id: input.secret_id,
        permissions: serde_json::to_string(&input.permissions).map_err(|_| ApiError::InvalidInput)?,
        created_at: now.clone(),
        updated_at: now,
    };
    inner.store.upsert_integration(&integration)?;
    Ok(Json(integration_response(integration)))
}

fn secret_response(secret: SecretRecord) -> SecretResponse {
    SecretResponse {
        id: secret.id,
        name: secret.name,
        key_version: secret.key_version,
        created_at: secret.created_at,
        updated_at: secret.updated_at,
    }
}

fn integration_response(integration: IntegrationRecord) -> IntegrationResponse {
    let permissions = serde_json::from_str(&integration.permissions).unwrap_or_default();
    IntegrationResponse {
        id: integration.id,
        provider: integration.provider,
        display_name: integration.display_name,
        secret_id: integration.secret_id,
        permissions,
        created_at: integration.created_at,
        updated_at: integration.updated_at,
    }
}

fn validate_secret_input(name: &str, value: &str) -> Result<(), ApiError> {
    if name.trim().is_empty()
        || name.len() > 120
        || value.is_empty()
        || value.len() > 64 * 1024
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::InvalidInput);
    }
    Ok(())
}

fn secret_cipher() -> Result<Aes256Gcm, ApiError> {
    let configured = std::env::var("SPECX_SECRET_KEY").map_err(|_| ApiError::InvalidInput)?;
    let key = Sha256::digest(configured.as_bytes());
    Aes256Gcm::new_from_slice(&key).map_err(|_| ApiError::InvalidInput)
}

fn encrypt_secret(value: &str) -> Result<String, ApiError> {
    let cipher = secret_cipher()?;
    let nonce_bytes = uuid::Uuid::new_v4().as_bytes()[..12].to_vec();
    let encrypted =
        cipher.encrypt(Nonce::from_slice(&nonce_bytes), value.as_bytes()).map_err(|_| ApiError::InvalidInput)?;
    Ok(format!("{}:{}", BASE64.encode(nonce_bytes), BASE64.encode(encrypted)))
}

fn decrypt_secret(value: &str) -> Result<String, ApiError> {
    let (nonce, payload) = value.split_once(':').ok_or(ApiError::InvalidInput)?;
    let nonce = BASE64.decode(nonce).map_err(|_| ApiError::InvalidInput)?;
    if nonce.len() != 12 {
        return Err(ApiError::InvalidInput);
    }
    let payload = BASE64.decode(payload).map_err(|_| ApiError::InvalidInput)?;
    let cipher = secret_cipher()?;
    let bytes = cipher.decrypt(Nonce::from_slice(&nonce), payload.as_ref()).map_err(|_| ApiError::InvalidInput)?;
    String::from_utf8(bytes).map_err(|_| ApiError::InvalidInput)
}

fn repository_credential(
    inner: &ApiInner,
    context: &WorkspaceContext,
    provider: &str,
) -> Result<Option<GitCredential>, ApiError> {
    let integrations = inner.store.list_integrations(&context.tenant_id, &context.workspace_id)?;
    let Some(integration) = integrations.into_iter().find(|integration| integration.provider == provider) else {
        return Ok(None);
    };
    let Some(secret_id) = integration.secret_id else { return Ok(None) };
    let permissions: Vec<String> =
        serde_json::from_str(&integration.permissions).map_err(|_| ApiError::InvalidInput)?;
    if !permissions.iter().any(|permission| permission == "repository:read" || permission == "contents:read") {
        return Ok(None);
    }
    let secret =
        inner.store.secret_by_id(&context.tenant_id, &context.workspace_id, &secret_id)?.ok_or(ApiError::NotFound)?;
    let token = decrypt_secret(&secret.encrypted_value)?;
    Ok(Some(GitCredential { username: None, bearer_token: Some(token) }))
}

fn verify_webhook_signature(secret: Option<&str>, signature: &str, body: &[u8]) -> bool {
    let Some(secret) = secret else { return false };
    let Some(expected) = signature.strip_prefix("sha256=") else { return false };
    let Ok(mut mac) = <hmac::Hmac<Sha256> as hmac::Mac>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(body);
    let expected_bytes = match hex::decode(expected) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    mac.verify_slice(&expected_bytes).is_ok()
}

pub(crate) async fn process_controller_task(state: &ApiState, task: &ControllerTask) -> Result<(), ApiError> {
    if task.controller_id != "repository-controller" || task.resource_kind != "Repository" {
        return Err(ApiError::InvalidInput);
    }
    let context = WorkspaceContext {
        tenant_id: task.tenant_id.clone(),
        workspace_id: task.workspace_id.clone(),
        actor_id: "specx-controller".into(),
    };
    let targets = {
        let mut inner = state.inner.lock().await;
        let core = load_core(&mut inner, &context)?;
        let repository = core.repository(&context, &task.resource_id).ok_or(CoreError::NotFound("repository"))?.clone();
        core.list_specifications(&context)
            .into_iter()
            .filter_map(|specification| {
                let binding = core.specification_code_binding(&context, &specification.id)?;
                let relation = core.service_repository(&context, &specification.service_id)?;
                (relation.repository_id == repository.id && !binding.path.is_empty()).then(|| {
                    (
                        specification.id.clone(),
                        binding.path.clone(),
                        binding.ref_kind,
                        binding.ref_name.clone(),
                        repository.canonical_url.clone(),
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    let mut refs = std::collections::BTreeMap::<(SpecificationRefKind, String), String>::new();
    for (_, _, ref_kind, ref_name, url) in &targets {
        refs.entry((*ref_kind, ref_name.clone())).or_insert_with(|| url.clone());
    }
    for ((ref_kind, ref_name), repository_url) in refs {
        let credential = {
            let mut inner = state.inner.lock().await;
            let core = load_core(&mut inner, &context)?;
            let repository = core.repository(&context, &task.resource_id).ok_or(CoreError::NotFound("repository"))?;
            repository_credential(&inner, &context, &repository.provider)?
        };
        let fetched = fetch_repository_snapshot(repository_url, ref_kind, ref_name.clone(), credential).await?;
        let synced_at = Utc::now().to_rfc3339();
        let snapshot =
            persist_repository_snapshot(state, &context, &task.resource_id, &ref_name, &fetched, &synced_at).await?;
        for (specification_id, path, binding_ref_kind, binding_ref_name, _) in
            targets.iter().filter(|target| target.2 == ref_kind && target.3 == ref_name)
        {
            let content = {
                let inner = state.inner.lock().await;
                read_repository_snapshot_file(state, &inner, &snapshot.id, path)?
            };
            let mut inner = state.inner.lock().await;
            let mut core = load_core(&mut inner, &context)?;
            let current = core
                .list_specifications(&context)
                .into_iter()
                .find(|value| value.id == *specification_id)
                .cloned()
                .ok_or(CoreError::NotFound("specification"))?;
            if current.content_hash == format!("sha256:{:x}", Sha256::digest(content.as_bytes())) {
                continue;
            }
            let updated = core.update_managed_specification_content(&context, specification_id, content)?;
            if !inner.store.update_specification_if_current_with_audit_and_snapshot(
                &context,
                &current,
                &updated,
                None,
                Some(&snapshot),
            )? {
                return Err(ApiError::PreconditionFailed);
            }
            inner.seed_core = None;
            let _ = binding_ref_kind;
            let _ = binding_ref_name;
        }
    }
    Ok(())
}

impl From<Specification> for SpecificationResponse {
    fn from(specification: Specification) -> Self {
        let mut response = Self {
            id: specification.id,
            tenant_id: specification.tenant_id,
            workspace_id: specification.workspace_id,
            service_id: specification.service_id,
            title: specification.title,
            kind: specification.kind.into(),
            content: specification.content,
            version: specification.version,
            content_hash: specification.content_hash,
            updated_at: specification.updated_at,
            etag: String::new(),
        };
        response.etag = specification_etag(&response);
        response
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogResponse {
    pub groups: Vec<ServiceGroupResponse>,
    pub tags: Vec<TagResponse>,
    pub services: Vec<ServiceResponse>,
    pub specifications: Vec<SpecificationResponse>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OffsetListQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

fn page_bounds(query: &OffsetListQuery, total: usize) -> Result<(usize, usize, Option<usize>), ApiError> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(ApiError::InvalidQuery);
    }
    let offset = query.offset.unwrap_or(0);
    let end = offset.saturating_add(limit).min(total);
    let next_offset = (end < total).then_some(end);
    Ok((limit, offset, next_offset))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupPageResponse {
    pub items: Vec<ServiceGroupResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagPageResponse {
    pub items: Vec<TagResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationPageResponse {
    pub items: Vec<SpecificationResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationRevisionResponse {
    pub specification_id: Id,
    pub version: i64,
    pub content: String,
    pub content_hash: String,
    pub created_by: Id,
    pub created_at: String,
}

impl From<SpecificationRevision> for SpecificationRevisionResponse {
    fn from(revision: SpecificationRevision) -> Self {
        Self {
            specification_id: revision.specification_id,
            version: revision.version,
            content: revision.content,
            content_hash: revision.content_hash,
            created_by: revision.created_by,
            created_at: revision.created_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationRevisionPageResponse {
    pub items: Vec<SpecificationRevisionResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/groups",
    params(OffsetListQuery),
    responses(
        (status = 200, description = "Stable paginated workspace groups", body = GroupPageResponse),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn list_groups(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<GroupPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut groups: Vec<_> = load_core(&mut inner, &context)?.list_groups(&context).into_iter().cloned().collect();
    groups.sort_by(|left, right| {
        left.parent_id
            .cmp(&right.parent_id)
            .then(left.position.cmp(&right.position))
            .then(left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then(left.id.cmp(&right.id))
    });
    let total = groups.len();
    let (limit, offset, next_offset) = page_bounds(&query, total)?;
    let items = groups.into_iter().skip(offset).take(limit).map(Into::into).collect();
    Ok(Json(GroupPageResponse { items, total, limit, offset, next_offset }))
}

#[utoipa::path(
    get,
    path = "/tags",
    params(OffsetListQuery),
    responses(
        (status = 200, description = "Stable paginated workspace tags", body = TagPageResponse),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn list_tags(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<TagPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut tags: Vec<_> = load_core(&mut inner, &context)?.list_tags(&context).into_iter().cloned().collect();
    tags.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()).then(left.id.cmp(&right.id)));
    let total = tags.len();
    let (limit, offset, next_offset) = page_bounds(&query, total)?;
    let items = tags.into_iter().skip(offset).take(limit).map(Into::into).collect();
    Ok(Json(TagPageResponse { items, total, limit, offset, next_offset }))
}

#[utoipa::path(
    get,
    path = "/specifications",
    params(OffsetListQuery),
    responses(
        (status = 200, description = "Stable paginated workspace specifications", body = SpecificationPageResponse),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn list_specifications(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<SpecificationPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut specifications: Vec<_> =
        load_core(&mut inner, &context)?.list_specifications(&context).into_iter().cloned().collect();
    specifications.sort_by(|left, right| right.updated_at.cmp(&left.updated_at).then(left.id.cmp(&right.id)));
    let total = specifications.len();
    let (limit, offset, next_offset) = page_bounds(&query, total)?;
    let items = specifications.into_iter().skip(offset).take(limit).map(Into::into).collect();
    Ok(Json(SpecificationPageResponse { items, total, limit, offset, next_offset }))
}

#[utoipa::path(
    get,
    path = "/specifications/{specification_id}/revisions",
    params(
        ("specification_id" = String, Path, description = "Stable specification identifier"),
        OffsetListQuery
    ),
    responses(
        (status = 200, description = "Immutable revisions, newest first", body = SpecificationRevisionPageResponse),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Specification is outside the authorized scope", body = ErrorResponse),
        (status = 404, description = "Specification does not exist", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn list_specification_revisions(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<SpecificationRevisionPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    if !load_core(&mut inner, &context)?
        .list_specifications(&context)
        .iter()
        .any(|specification| specification.id == specification_id)
    {
        return Err(CoreError::NotFound("specification").into());
    }
    let total = inner.store.specification_revision_count(&context, &specification_id)? as usize;
    let (limit, offset, next_offset) = page_bounds(&query, total)?;
    let items = inner
        .store
        .specification_revisions(&context, &specification_id, limit, offset)?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(SpecificationRevisionPageResponse { items, total, limit, offset, next_offset }))
}

#[utoipa::path(
    get,
    path = "/specifications/{specification_id}/revisions/{version}",
    params(
        ("specification_id" = String, Path, description = "Stable specification identifier"),
        ("version" = i64, Path, description = "Immutable revision number")
    ),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 200, description = "Immutable specification revision", body = SpecificationRevisionResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Specification is outside the authorized scope", body = ErrorResponse),
        (status = 404, description = "Revision does not exist", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn get_specification_revision(
    State(state): State<ApiState>,
    Path((specification_id, version)): Path<(Id, i64)>,
    headers: HeaderMap,
) -> Result<Json<SpecificationRevisionResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    inner
        .store
        .specification_revision(&context, &specification_id, version)?
        .map(Into::into)
        .map(Json)
        .ok_or_else(|| CoreError::NotFound("revision").into())
}

#[utoipa::path(
    post,
    path = "/specifications/{specification_id}/revisions/{version}/restore",
    params(
        ("specification_id" = String, Path, description = "Stable specification identifier"),
        ("version" = i64, Path, description = "Immutable revision number to restore"),
        ("If-Match" = String, Header, description = "Strong ETag from the current specification representation")
    ),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 200, description = "Restored content as a newly appended revision", body = SpecificationResponse,
            headers(("ETag" = String, description = "New strong validator for subsequent conditional updates"))),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Specification is outside the authorized scope", body = ErrorResponse),
        (status = 404, description = "Specification or revision does not exist", body = ErrorResponse),
        (status = 412, description = "If-Match does not match the current specification", body = ErrorResponse),
        (status = 428, description = "If-Match is required", body = ErrorResponse),
        (status = 409, description = "Repository-managed specifications cannot be restored", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn restore_specification_revision(
    State(state): State<ApiState>,
    Path((specification_id, version)): Path<(Id, i64)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let expected_etags: Vec<_> = headers.get_all(IF_MATCH).iter().filter_map(|value| value.to_str().ok()).collect();
    if expected_etags.is_empty() {
        return Err(ApiError::PreconditionRequired);
    }
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let existing = core
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let existing_response = SpecificationResponse::from(existing.clone());
    let current_etag = specification_etag(&existing_response);
    if !expected_etags.iter().any(|if_match| if_match_satisfies(if_match, &current_etag)) {
        return Err(ApiError::PreconditionFailed);
    }
    let revision = inner
        .store
        .specification_revision(&context, &specification_id, version)?
        .ok_or(CoreError::NotFound("revision"))?;
    let updated =
        core.update_markdown_specification(&context, &specification_id, existing.title.clone(), revision.content)?;
    let audit = audit_event(&context, &headers, "restore", "specification", &specification_id);
    if !inner.store.update_specification_if_current_with_audit(&context, &existing, &updated, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let restored = inner
        .store
        .load_core(&context)?
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let response = SpecificationResponse::from(restored);
    let etag = specification_etag(&response);
    Ok(response_with_etag(response, etag))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServicePageResponse {
    pub items: Vec<ServiceResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub next_cursor: Option<String>,
    pub query: String,
    pub tag_ids: Vec<Id>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceListQuery {
    pub q: Option<String>,
    pub tag_id: Option<Id>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub cursor: Option<String>,
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/catalog",
    params(
        ("x-specx-tenant-id" = String, Header, description = "Authenticated tenant context"),
        ("x-specx-workspace-id" = String, Header, description = "Selected authorized workspace"),
        ("x-specx-actor-id" = String, Header, description = "Authenticated actor context")
    ),
    responses(
        (status = 200, description = "Workspace catalog", body = CatalogResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn get_catalog(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<CatalogResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    Ok(Json(CatalogResponse {
        groups: core.list_groups(&context).into_iter().cloned().map(Into::into).collect(),
        tags: core.list_tags(&context).into_iter().cloned().map(Into::into).collect(),
        services: core.list_services(&context).into_iter().cloned().map(Into::into).collect(),
        specifications: core.list_specifications(&context).into_iter().cloned().map(Into::into).collect(),
    }))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Owner,
    Admin,
    Editor,
    Viewer,
}

impl WorkspaceRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetMemberRoleRequest {
    pub role: WorkspaceRole,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberResponse {
    pub actor_id: Id,
    pub role: String,
    pub revoked_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberPageResponse {
    pub items: Vec<WorkspaceMemberResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/members",
    params(OffsetListQuery),
    responses(
        (status = 200, description = "Stable paginated workspace members", body = WorkspaceMemberPageResponse),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Member administration requires owner or admin role", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn list_members(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<WorkspaceMemberPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    let mut members: Vec<_> = inner
        .store
        .workspace_members(&context)?
        .into_iter()
        .map(|member| WorkspaceMemberResponse {
            actor_id: member.actor_id,
            role: member.role,
            revoked_at: member.revoked_at,
            created_at: member.created_at,
        })
        .collect();
    members.sort_by(|left, right| left.actor_id.cmp(&right.actor_id));
    let total = members.len();
    let (limit, offset, next_offset) = page_bounds(&query, total)?;
    let items = members.into_iter().skip(offset).take(limit).collect();
    Ok(Json(WorkspaceMemberPageResponse { items, total, limit, offset, next_offset }))
}

#[utoipa::path(
    put,
    path = "/members/{member_actor_id}",
    params(("member_actor_id" = String, Path, description = "Authenticated actor identifier to grant access to")),
    request_body = SetMemberRoleRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Workspace member role updated", body = WorkspaceMemberResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Insufficient role to manage this member", body = ErrorResponse),
        (status = 409, description = "The last workspace owner cannot be demoted", body = ErrorResponse),
        (status = 422, description = "Invalid role", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn set_member_role(
    State(state): State<ApiState>,
    Path(member_actor_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<SetMemberRoleRequest>,
) -> Result<Json<WorkspaceMemberResponse>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let actor_role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&actor_role) || (input.role == WorkspaceRole::Owner && actor_role != "owner") {
        return Err(ApiError::Forbidden);
    }
    let audit = audit_event(&context, &headers, "grant", "workspace_member", &member_actor_id);
    if !inner.store.set_workspace_member_role(&context, &member_actor_id, input.role.as_str(), &audit)? {
        return Err(ApiError::LastOwnerRequired);
    }
    let member = inner
        .store
        .workspace_members(&context)?
        .into_iter()
        .find(|member| member.actor_id == member_actor_id)
        .ok_or(CoreError::NotFound("workspace_member"))?;
    Ok(Json(WorkspaceMemberResponse {
        actor_id: member.actor_id,
        role: member.role,
        revoked_at: member.revoked_at,
        created_at: member.created_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/members/{member_actor_id}",
    params(("member_actor_id" = String, Path, description = "Workspace member actor identifier")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Workspace member revoked"),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Insufficient role to manage this member", body = ErrorResponse),
        (status = 404, description = "Workspace member not found", body = ErrorResponse),
        (status = 409, description = "The last workspace owner cannot be revoked", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn revoke_member(
    State(state): State<ApiState>,
    Path(member_actor_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let actor_role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&actor_role) {
        return Err(ApiError::Forbidden);
    }
    let member_role = inner
        .store
        .workspace_member_role(&context, &member_actor_id)?
        .ok_or(CoreError::NotFound("workspace_member"))?;
    if actor_role != "owner" && member_role == "owner" {
        return Err(ApiError::Forbidden);
    }
    let audit = audit_event(&context, &headers, "revoke", "workspace_member", &member_actor_id);
    if !inner.store.revoke_workspace_member_with_audit(&context, &member_actor_id, &audit)? {
        return Err(ApiError::LastOwnerRequired);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptInvitationRequest {
    pub token: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitationResponse {
    pub id: Id,
    pub email: String,
    pub role: String,
    pub expires_at: String,
    pub accepted_at: Option<String>,
    pub revoked_at: Option<String>,
    pub created_by: Id,
    pub created_at: String,
}

impl From<WorkspaceInvitation> for InvitationResponse {
    fn from(invitation: WorkspaceInvitation) -> Self {
        Self {
            id: invitation.id,
            email: invitation.email,
            role: invitation.role,
            expires_at: invitation.expires_at,
            accepted_at: invitation.accepted_at,
            revoked_at: invitation.revoked_at,
            created_by: invitation.created_by,
            created_at: invitation.created_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatedInvitationResponse {
    #[serde(flatten)]
    pub invitation: InvitationResponse,
    pub token: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedInvitationResponse {
    pub workspace_id: Id,
    pub actor_id: Id,
    pub role: String,
}

#[utoipa::path(
    get,
    path = "/invitations",
    responses(
        (status = 200, description = "Workspace invitations; bearer tokens are never listed", body = [InvitationResponse]),
        (status = 403, description = "Invitation administration requires owner or admin role", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn list_invitations(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<InvitationResponse>>, ApiError> {
    let context = request_context(&headers)?;
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(inner.store.workspace_invitations(&context)?.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    post,
    path = "/invitations",
    request_body = CreateInvitationRequest,
    responses(
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 201, description = "Invitation created; token is returned once for delivery", body = CreatedInvitationResponse),
        (status = 400, body = ErrorResponse),
        (status = 403, description = "Invitation administration requires owner or admin role", body = ErrorResponse),
        (status = 422, body = ErrorResponse)
    ),
    tag = "members"
)]
async fn create_invitation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitationResponse>), ApiError> {
    let context = request_context(&headers)?;
    let email = input.email.trim().to_ascii_lowercase();
    if email.len() > 254
        || email.chars().any(char::is_whitespace)
        || email.matches('@').count() != 1
        || email.split_once('@').is_none_or(|(local, domain)| local.is_empty() || !domain.contains('.'))
    {
        return Err(ApiError::InvalidInput);
    }
    let actor_role = {
        let inner = state.inner.lock().await;
        inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?
    };
    if !can_manage_members(&actor_role) || (input.role == WorkspaceRole::Owner && actor_role != "owner") {
        return Err(ApiError::Forbidden);
    }
    let now = Utc::now();
    let invitation = WorkspaceInvitation {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        email,
        role: input.role.as_str().to_owned(),
        token_hash: String::new(),
        expires_at: (now + chrono::Duration::days(7)).to_rfc3339(),
        accepted_at: None,
        created_by: context.actor_id.clone(),
        created_at: now.to_rfc3339(),
        revoked_at: None,
    };
    let token = format!("{}.{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let token_hash = format!("sha256:{:x}", Sha256::digest(token.as_bytes()));
    let invitation = WorkspaceInvitation { token_hash, ..invitation };
    let audit = audit_event(&context, &headers, "invite", "workspace_invitation", &invitation.id);
    let inner = state.inner.lock().await;
    if !inner.store.create_workspace_invitation(&context, &invitation, &audit)? {
        return Err(ApiError::Forbidden);
    }
    let response = CreatedInvitationResponse { invitation: invitation.into(), token };
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    delete,
    path = "/invitations/{invitation_id}",
    params(("invitation_id" = String, Path, description = "Invitation identifier")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Invitation revoked"),
        (status = 401, body = ErrorResponse),
        (status = 403, description = "Invitation administration requires owner or admin role", body = ErrorResponse),
        (status = 404, description = "Invitation is missing, already accepted, or already revoked", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn revoke_invitation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(invitation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    if invitation_id.trim().is_empty() || invitation_id.len() > 128 {
        return Err(ApiError::NotFound);
    }
    let now = Utc::now().to_rfc3339();
    let audit = audit_event(&context, &headers, "revoke", "workspace_invitation", &invitation_id);
    let inner = state.inner.lock().await;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    if !inner.store.revoke_workspace_invitation(&context, &invitation_id, &now, &audit)? {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/invitations/accept",
    request_body = AcceptInvitationRequest,
    responses(
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 422, description = "Invalid request data", body = ErrorResponse),
        (status = 200, description = "Invitation accepted and workspace membership created", body = AcceptedInvitationResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, description = "Invitation is invalid, expired, or already accepted", body = ErrorResponse)
    ),
    tag = "members"
)]
async fn accept_invitation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<AcceptInvitationRequest>,
) -> Result<Json<AcceptedInvitationResponse>, ApiError> {
    if input.token.len() > 256 || input.token.is_empty() {
        return Err(ApiError::InvalidInput);
    }
    let actor = invitation_actor_context(&headers)?;
    let token_hash = format!("sha256:{:x}", Sha256::digest(input.token.as_bytes()));
    let inner = state.inner.lock().await;
    let invitation = inner.store.find_workspace_invitation(&actor.tenant_id, &token_hash)?.ok_or(ApiError::NotFound)?;
    let context = WorkspaceContext {
        tenant_id: actor.tenant_id,
        workspace_id: invitation.workspace_id.clone(),
        actor_id: actor.actor_id,
    };
    let now = Utc::now().to_rfc3339();
    let audit = audit_event(&context, &headers, "accept_invitation", "workspace_invitation", &invitation.id);
    let member =
        inner.store.accept_workspace_invitation(&context, &token_hash, &now, &audit)?.ok_or(ApiError::NotFound)?;
    Ok(Json(AcceptedInvitationResponse {
        workspace_id: context.workspace_id,
        actor_id: member.actor_id,
        role: member.role,
    }))
}

#[utoipa::path(
    get,
    path = "/services",
    params(ServiceListQuery),
    responses(
        (status = 200, description = "Filtered and paginated workspace services", body = ServicePageResponse),
        (status = 400, description = "Invalid pagination or filter", body = ErrorResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn list_services(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ServiceListQuery>,
) -> Result<Json<ServicePageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let search = query.q.unwrap_or_default().trim().to_lowercase();
    let tag_ids: Vec<_> = query.tag_id.into_iter().collect();
    let include_archived = query.include_archived.unwrap_or(false);
    let limit = query.limit.unwrap_or(50);
    if search.len() > 256
        || tag_ids.iter().any(|tag_id| tag_id.len() > 128)
        || !(1..=200).contains(&limit)
        || (query.cursor.is_some() && query.offset.is_some())
    {
        return Err(ApiError::InvalidQuery);
    }
    let offset = match query.cursor.as_deref() {
        Some(cursor) => {
            decode_service_cursor(cursor, &search, &tag_ids, include_archived).ok_or(ApiError::InvalidQuery)?
        }
        None => query.offset.unwrap_or(0),
    };

    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let mut services: Vec<_> = core
        .list_services(&context)
        .into_iter()
        .filter(|service| {
            (include_archived || !service.archived)
                && (search.is_empty()
                    || service.name.to_lowercase().contains(&search)
                    || service.slug.to_lowercase().contains(&search)
                    || service.description.to_lowercase().contains(&search))
                && tag_ids.iter().all(|tag_id| service.tag_ids.contains(tag_id))
        })
        .cloned()
        .collect();
    services.sort_by(|left, right| left.slug.cmp(&right.slug).then_with(|| left.id.cmp(&right.id)));
    let total = services.len();
    let page_end = offset.saturating_add(limit).min(total);
    let items = if offset >= total { Vec::new() } else { services[offset..page_end].to_vec() };
    let next_offset = (page_end < total).then_some(page_end);
    let next_cursor = next_offset.map(|next| encode_service_cursor(&search, &tag_ids, include_archived, next));

    Ok(Json(ServicePageResponse {
        items: items.into_iter().map(Into::into).collect(),
        total,
        limit,
        offset,
        next_offset,
        next_cursor,
        query: search,
        tag_ids,
    }))
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateGroupRequest {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    #[schema(required = false)]
    pub parent_id: Option<Id>,
    #[serde(default)]
    #[schema(required = false)]
    pub position: Option<i64>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateGroupRequest {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<Id>,
    #[serde(default)]
    #[schema(required = false)]
    pub position: Option<i64>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupOrderItem {
    pub id: Id,
    pub parent_id: Option<Id>,
    pub position: i64,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReorderGroupsRequest {
    pub items: Vec<GroupOrderItem>,
}

#[utoipa::path(
    post,
    path = "/groups",
    params(("Idempotency-Key" = Option<String>, Header, description = "Client supplied key for safe retries")),
    request_body = CreateGroupRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Group created", body = ServiceGroupResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Parent group is outside the authorized scope", body = ErrorResponse),
        (status = 409, description = "Idempotency key was already used with a different request", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 422, description = "Invalid group", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn create_group(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateGroupRequest>,
) -> Result<Json<ServiceGroup>, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "groups", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "service_group" {
            return Err(ApiError::IdempotencyConflict);
        }
        let core = load_core(&mut inner, &context)?;
        let group = core
            .list_groups(&context)
            .into_iter()
            .find(|group| group.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        return Ok(Json(group));
    }
    let mut core = load_core(&mut inner, &context)?;
    let group = match input.position {
        Some(position) => core.create_group_with_position(&context, input.id, input.name, input.parent_id, position)?,
        None => core.create_group(&context, input.id, input.name, input.parent_id)?,
    };
    let resource_id = group.id.clone();
    let audit = audit_event(&context, &headers, "create", "service_group", &resource_id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "service_group", &resource_id),
    )?;
    inner.seed_core = None;
    Ok(Json(group))
}

#[utoipa::path(
    put,
    path = "/groups/{group_id}",
    params(("group_id" = String, Path, description = "Stable group identifier")),
    request_body = UpdateGroupRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Group updated or moved", body = ServiceGroupResponse),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 422, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn update_group(
    State(state): State<ApiState>,
    Path(group_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateGroupRequest>,
) -> Result<Json<ServiceGroupResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let group = match input.position {
        Some(position) => {
            core.update_group_with_position(&context, &group_id, input.name, input.parent_id, position)?
        }
        None => core.update_group(&context, &group_id, input.name, input.parent_id)?,
    };
    let audit = audit_event(&context, &headers, "update", "service_group", &group_id);
    inner.store.persist_core_with_audit(&core, &context, Some(&audit))?;
    inner.seed_core = None;
    Ok(Json(group.into()))
}

#[utoipa::path(
    put,
    path = "/groups/order",
    request_body = ReorderGroupsRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Group hierarchy and order saved", body = [ServiceGroupResponse]),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 422, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn reorder_groups(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<ReorderGroupsRequest>,
) -> Result<Json<Vec<ServiceGroupResponse>>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let items = input
        .items
        .into_iter()
        .map(|item| ServiceGroupOrder { id: item.id, parent_id: item.parent_id, position: item.position })
        .collect::<Vec<_>>();
    core.reorder_groups(&context, &items)?;
    let audit = audit_event(&context, &headers, "reorder", "service_group", &context.workspace_id);
    inner.store.persist_core_with_audit(&core, &context, Some(&audit))?;
    inner.seed_core = None;
    let mut groups: Vec<_> = core.list_groups(&context).into_iter().cloned().map(Into::into).collect();
    groups.sort_by(|left: &ServiceGroupResponse, right| {
        left.parent_id.cmp(&right.parent_id).then(left.position.cmp(&right.position)).then(left.name.cmp(&right.name))
    });
    Ok(Json(groups))
}

#[utoipa::path(
    delete,
    path = "/groups/{group_id}",
    params(("group_id" = String, Path, description = "Stable group identifier")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Group deleted"),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn delete_group(
    State(state): State<ApiState>,
    Path(group_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    core.delete_group(&context, &group_id)?;
    let audit = audit_event(&context, &headers, "delete", "service_group", &group_id);
    inner.store.delete_group(&context, &group_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTagRequest {
    pub id: Id,
    pub name: String,
    #[serde(default = "default_tag_color")]
    #[schema(default = "#64748b", required = false)]
    pub color: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTagRequest {
    pub name: String,
    pub color: String,
}

fn default_tag_color() -> String {
    "#64748b".into()
}

#[utoipa::path(
    post,
    path = "/tags",
    params(("Idempotency-Key" = Option<String>, Header, description = "Client supplied key for safe retries")),
    request_body = CreateTagRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Tag created", body = TagResponse),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Actor is not a workspace member", body = ErrorResponse),
        (status = 409, description = "Idempotency key was already used with a different request", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 422, description = "Invalid tag", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn create_tag(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateTagRequest>,
) -> Result<Json<Tag>, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "tags", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "tag" {
            return Err(ApiError::IdempotencyConflict);
        }
        let core = load_core(&mut inner, &context)?;
        let tag = core
            .list_tags(&context)
            .into_iter()
            .find(|tag| tag.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        return Ok(Json(tag));
    }
    let mut core = load_core(&mut inner, &context)?;
    let tag = core.create_tag(&context, input.id, input.name, input.color)?;
    let resource_id = tag.id.clone();
    let audit = audit_event(&context, &headers, "create", "tag", &resource_id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "tag", &resource_id),
    )?;
    inner.seed_core = None;
    Ok(Json(tag))
}

#[utoipa::path(
    put,
    path = "/tags/{tag_id}",
    params(("tag_id" = String, Path, description = "Stable tag identifier")),
    request_body = UpdateTagRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Tag updated", body = TagResponse),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 422, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn update_tag(
    State(state): State<ApiState>,
    Path(tag_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let tag = core.update_tag(&context, &tag_id, input.name, input.color)?;
    let audit = audit_event(&context, &headers, "update", "tag", &tag_id);
    inner.store.persist_core_with_audit(&core, &context, Some(&audit))?;
    inner.seed_core = None;
    Ok(Json(tag.into()))
}

#[utoipa::path(
    delete,
    path = "/tags/{tag_id}",
    params(("tag_id" = String, Path, description = "Stable tag identifier")),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Tag deleted"),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn delete_tag(
    State(state): State<ApiState>,
    Path(tag_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    core.delete_tag(&context, &tag_id)?;
    let audit = audit_event(&context, &headers, "delete", "tag", &tag_id);
    inner.store.delete_tag(&context, &tag_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateServiceRequest {
    pub id: Id,
    #[schema(required = false)]
    pub group_id: Option<Id>,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    #[schema(required = false)]
    pub description: String,
    #[serde(default)]
    #[schema(required = false)]
    pub tag_ids: Vec<Id>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateServiceRequest {
    #[schema(required = false)]
    pub group_id: Option<Id>,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    #[schema(required = false)]
    pub description: String,
    #[serde(default)]
    #[schema(required = false)]
    pub tag_ids: Vec<Id>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetServiceArchivedRequest {
    pub archived: bool,
}

#[utoipa::path(
    post,
    path = "/services",
    params(("Idempotency-Key" = Option<String>, Header, description = "Client supplied key for safe retries")),
    request_body = CreateServiceRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Service created", body = ServiceResponse, headers(("ETag" = String, description = "Strong validator for subsequent updates"))),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Reference is outside the authorized scope", body = ErrorResponse),
        (status = 409, description = "Idempotency key was already used with a different request", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 404, description = "Referenced resource does not exist", body = ErrorResponse),
        (status = 409, description = "Conflicting service tags", body = ErrorResponse),
        (status = 422, description = "Invalid service", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn create_service(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateServiceRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "services", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "service" {
            return Err(ApiError::IdempotencyConflict);
        }
        let core = load_core(&mut inner, &context)?;
        let service = core
            .list_services(&context)
            .into_iter()
            .find(|service| service.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        let response: ServiceResponse = service.into();
        let etag = response.etag.clone();
        return Ok(response_with_etag(response, etag));
    }
    let mut core = load_core(&mut inner, &context)?;
    let service = core.create_service(
        &context,
        Service {
            id: input.id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            group_id: input.group_id,
            name: input.name,
            slug: input.slug,
            description: input.description,
            tag_ids: input.tag_ids,
            updated_at: Utc::now().to_rfc3339(),
            archived: false,
        },
    )?;
    let resource_id = service.id.clone();
    let audit = audit_event(&context, &headers, "create", "service", &resource_id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "service", &resource_id),
    )?;
    inner.seed_core = None;
    let response: ServiceResponse = service.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    put,
    path = "/services/{service_id}",
    params(
        ("service_id" = String, Path, description = "Stable service ID"),
        ("If-Match" = String, Header, description = "Strong ETag from the current service representation")
    ),
    request_body = UpdateServiceRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 200, description = "Service updated or moved", body = ServiceResponse, headers(("ETag" = String, description = "New strong validator for subsequent updates"))),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Service or reference is outside the authorized scope", body = ErrorResponse),
        (status = 404, description = "Service or referenced resource does not exist", body = ErrorResponse),
        (status = 409, description = "Service slug is already in use", body = ErrorResponse),
        (status = 412, description = "If-Match does not match the current service", body = ErrorResponse),
        (status = 428, description = "If-Match is required", body = ErrorResponse),
        (status = 422, description = "Invalid service", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn update_service(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateServiceRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let expected = core
        .list_services(&context)
        .into_iter()
        .find(|service| service.id == service_id)
        .cloned()
        .ok_or(CoreError::NotFound("service"))?;
    let current_etag = ServiceResponse::from(expected.clone()).etag;
    let expected_etags: Vec<_> = headers.get_all(IF_MATCH).iter().filter_map(|value| value.to_str().ok()).collect();
    if expected_etags.is_empty() {
        return Err(ApiError::PreconditionRequired);
    }
    if !expected_etags.iter().any(|if_match| if_match_satisfies(if_match, &current_etag)) {
        return Err(ApiError::PreconditionFailed);
    }
    let service = core.update_service(
        &context,
        &service_id,
        input.name,
        input.slug,
        input.description,
        input.group_id,
        input.tag_ids,
    )?;
    let audit = audit_event(&context, &headers, "update", "service", &service_id);
    if !inner.store.update_service_if_current_with_audit(&context, &expected, &service, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let response: ServiceResponse = service.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    patch,
    path = "/services/{service_id}",
    params(("service_id" = String, Path, description = "Stable service ID")),
    request_body = SetServiceArchivedRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (status = 422, description = "Invalid request data", body = ErrorResponse),
        (status = 200, description = "Service archived or restored", body = ServiceResponse, headers(("ETag" = String, description = "Current strong validator for the service"))),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Service is outside the authorized scope", body = ErrorResponse),
        (status = 404, description = "Service does not exist", body = ErrorResponse),
        (status = 412, description = "Service changed during the archive update", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn set_service_archived(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<SetServiceArchivedRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let expected = core
        .list_services(&context)
        .into_iter()
        .find(|service| service.id == service_id)
        .cloned()
        .ok_or(CoreError::NotFound("service"))?;
    let service = core.set_service_archived(&context, &service_id, input.archived)?;
    let action = if input.archived { "archive" } else { "restore" };
    let audit = audit_event(&context, &headers, action, "service", &service_id);
    if !inner.store.update_service_if_current_with_audit(&context, &expected, &service, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let response: ServiceResponse = service.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    delete,
    path = "/services/{service_id}",
    params(
        ("service_id" = String, Path, description = "Stable service ID"),
        ("If-Match" = String, Header, description = "Strong ETag from the current service representation")
    ),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Service and its specifications deleted"),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 412, body = ErrorResponse),
        (status = 428, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn delete_service(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let service = core
        .list_services(&context)
        .into_iter()
        .find(|service| service.id == service_id)
        .cloned()
        .ok_or(CoreError::NotFound("service"))?;
    let current_etag = ServiceResponse::from(service).etag;
    let expected_etags: Vec<_> = headers.get_all(IF_MATCH).iter().filter_map(|value| value.to_str().ok()).collect();
    if expected_etags.is_empty() {
        return Err(ApiError::PreconditionRequired);
    }
    if !expected_etags.iter().any(|if_match| if_match_satisfies(if_match, &current_etag)) {
        return Err(ApiError::PreconditionFailed);
    }
    let audit = audit_event(&context, &headers, "delete", "service", &service_id);
    inner.store.delete_service(&context, &service_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCodeSourceRequest {
    pub id: Id,
    #[schema(value_type = String)]
    pub source_type: CodeSourceType,
    pub provider: String,
    pub display_name: String,
    pub canonical_key: String,
    pub root_locator: String,
    #[schema(value_type = String)]
    pub execution_scope: CodeExecutionScope,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCodeSourceRequest {
    #[schema(value_type = String)]
    pub source_type: CodeSourceType,
    pub provider: String,
    pub display_name: String,
    pub canonical_key: String,
    pub root_locator: String,
    #[schema(value_type = String)]
    pub execution_scope: CodeExecutionScope,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeSourcePageResponse {
    pub items: Vec<CodeSourceResponse>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCodeBindingRequest {
    pub id: Id,
    pub source_id: Id,
    #[schema(value_type = String)]
    pub role: CodeBindingRole,
    #[schema(value_type = String)]
    pub entity_kind: CodeEntityKind,
    #[serde(default)]
    pub path: String,
    #[schema(value_type = String)]
    pub revision_kind: CodeRevisionKind,
    pub revision: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCodeBindingRequest {
    #[schema(value_type = String)]
    pub role: CodeBindingRole,
    #[schema(value_type = String)]
    pub entity_kind: CodeEntityKind,
    #[serde(default)]
    pub path: String,
    #[schema(value_type = String)]
    pub revision_kind: CodeRevisionKind,
    pub revision: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCodeBindingPageResponse {
    pub items: Vec<ServiceCodeBindingResponse>,
}

#[utoipa::path(get, path = "/code-sources", responses((status = 200, body = CodeSourcePageResponse)), tag = "code")]
async fn list_code_sources(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<CodeSourcePageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut items: Vec<_> =
        load_core(&mut inner, &context)?.list_code_sources(&context).into_iter().cloned().map(Into::into).collect();
    items.sort_by(|left: &CodeSourceResponse, right: &CodeSourceResponse| {
        left.display_name.cmp(&right.display_name).then_with(|| left.id.cmp(&right.id))
    });
    Ok(Json(CodeSourcePageResponse { items }))
}

#[utoipa::path(post, path = "/code-sources", params(("Idempotency-Key" = Option<String>, Header)), request_body = CreateCodeSourceRequest, responses((status = 200, body = CodeSourceResponse, headers(("ETag" = String, description = "Strong validator for subsequent updates")))), tag = "code")]
async fn create_code_source(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateCodeSourceRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "code-sources", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "code_source" {
            return Err(ApiError::IdempotencyConflict);
        }
        let response = load_core(&mut inner, &context)?
            .list_code_sources(&context)
            .into_iter()
            .find(|source| source.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        let response: CodeSourceResponse = response.into();
        let etag = response.etag.clone();
        return Ok(response_with_etag(response, etag));
    }
    let now = Utc::now().to_rfc3339();
    let mut core = load_core(&mut inner, &context)?;
    let source = core.create_code_source(
        &context,
        CodeSource {
            id: input.id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            source_type: input.source_type,
            provider: input.provider,
            display_name: input.display_name,
            canonical_key: input.canonical_key,
            root_locator: input.root_locator,
            execution_scope: input.execution_scope,
            created_at: now.clone(),
            updated_at: now,
            archived: false,
        },
    )?;
    let id = source.id.clone();
    let audit = audit_event(&context, &headers, "create", "code_source", &id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "code_source", &id),
    )?;
    inner.seed_core = None;
    let response: CodeSourceResponse = source.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(put, path = "/code-sources/{source_id}", params(("source_id" = String, Path), ("If-Match" = String, Header)), request_body = UpdateCodeSourceRequest, responses((status = 200, body = CodeSourceResponse, headers(("ETag" = String, description = "New strong validator for subsequent updates")))), tag = "code")]
async fn update_code_source(
    State(state): State<ApiState>,
    Path(source_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateCodeSourceRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let expected = core
        .list_code_sources(&context)
        .into_iter()
        .find(|source| source.id == source_id)
        .cloned()
        .ok_or(CoreError::NotFound("code_source"))?;
    let current = CodeSourceResponse::from(expected.clone()).etag;
    let matches = headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current));
    if !matches {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    let source = core.update_code_source(
        &context,
        &source_id,
        input.source_type,
        input.display_name,
        input.provider,
        input.canonical_key,
        input.root_locator,
        input.execution_scope,
    )?;
    let audit = audit_event(&context, &headers, "update", "code_source", &source_id);
    if !inner.store.update_code_source_if_current_with_audit(&context, &expected, &source, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let response: CodeSourceResponse = source.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(delete, path = "/code-sources/{source_id}", params(("source_id" = String, Path), ("If-Match" = String, Header)), responses((status = 204)), tag = "code")]
async fn delete_code_source(
    State(state): State<ApiState>,
    Path(source_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let source = core
        .list_code_sources(&context)
        .into_iter()
        .find(|source| source.id == source_id)
        .cloned()
        .ok_or(CoreError::NotFound("code_source"))?;
    let current = CodeSourceResponse::from(source.clone()).etag;
    let matches = headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current));
    if !matches {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    let mut next = core;
    next.delete_code_source(&context, &source_id)?;
    let audit = audit_event(&context, &headers, "delete", "code_source", &source_id);
    inner.store.delete_code_source(&context, &source_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/services/{service_id}/repository",
    params(("service_id" = String, Path)),
    responses((status = 200, body = ServiceRepositoryResponse, headers(("ETag" = String))), (status = 404, body = ErrorResponse)),
    tag = "code"
)]
async fn get_service_repository(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    if !core.list_services(&context).into_iter().any(|service| service.id == service_id) {
        return Err(CoreError::NotFound("service").into());
    }
    let response = service_repository_response(&core, &context, &service_id)?;
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    get,
    path = "/specifications/{specification_id}/code-binding",
    params(("specification_id" = String, Path)),
    responses((status = 200, body = SpecificationCodeBindingResponse, headers(("ETag" = String))), (status = 404, body = ErrorResponse)),
    tag = "code"
)]
async fn get_specification_code_binding(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let binding = core
        .specification_code_binding(&context, &specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification_code_binding"))?;
    let response = specification_code_binding_response(&core, &context, &binding)?;
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    put,
    path = "/specifications/{specification_id}/code-binding",
    params(("specification_id" = String, Path), ("If-Match" = Option<String>, Header)),
    request_body = SetSpecificationCodeBindingRequest,
    responses((status = 200, body = SpecificationCodeBindingResponse, headers(("ETag" = String))), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 412, body = ErrorResponse), (status = 422, body = ErrorResponse), (status = 428, body = ErrorResponse)),
    tag = "code"
)]
async fn set_specification_code_binding(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<SetSpecificationCodeBindingRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let specification = core
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let relation = core
        .service_repository(&context, &specification.service_id)
        .ok_or(CoreError::NotFound("service_repository"))?;
    let repository_id = relation.repository_id.clone();
    let existing = core.specification_code_binding(&context, &specification_id).cloned();
    if let Some(existing) = &existing {
        let current = specification_code_binding_response(&core, &context, existing)?.etag;
        let matches = headers
            .get_all(IF_MATCH)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .any(|value| if_match_satisfies(value, &current));
        if !matches {
            return Err(if headers.get(IF_MATCH).is_none() {
                ApiError::PreconditionRequired
            } else {
                ApiError::PreconditionFailed
            });
        }
    }
    let now = Utc::now().to_rfc3339();
    let binding = core.set_specification_code_binding(
        &context,
        SpecificationCodeBinding {
            id: existing.as_ref().map_or_else(|| uuid::Uuid::new_v4().to_string(), |binding| binding.id.clone()),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            specification_id: specification_id.clone(),
            service_repository_id: relation.id.clone(),
            path: input.path,
            ref_kind: input.ref_kind,
            ref_name: input.ref_name,
            created_at: existing.as_ref().map_or_else(|| now.clone(), |binding| binding.created_at.clone()),
            updated_at: now,
        },
    )?;
    let audit = audit_event(
        &context,
        &headers,
        if existing.is_some() { "update" } else { "create" },
        "specification_code_binding",
        &binding.id,
    );
    let persisted = if let Some(expected) = existing {
        inner.store.update_specification_code_binding_if_current_with_audit(
            &context,
            &expected,
            &binding,
            Some(&audit),
        )?
    } else {
        inner.store.persist_core_with_audit(&core, &context, Some(&audit))?;
        true
    };
    if !persisted {
        return Err(ApiError::PreconditionFailed);
    }
    if !binding.path.is_empty() && binding.ref_kind == SpecificationRefKind::Branch {
        upsert_repository_schedule(&inner.store, &context, &repository_id, binding.ref_kind, &binding.ref_name)?;
    }
    inner.seed_core = None;
    let response = specification_code_binding_response(&core, &context, &binding)?;
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

fn upsert_repository_schedule(
    store: &StoreBackend,
    context: &WorkspaceContext,
    service_repository_id: &str,
    ref_kind: SpecificationRefKind,
    ref_name: &str,
) -> Result<(), ApiError> {
    let interval_seconds = std::env::var("SPECX_REPOSITORY_POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(300)
        .max(30);
    let now = Utc::now();
    let schedule = RepositorySyncSchedule {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        repository_id: service_repository_id.to_owned(),
        ref_kind: serde_json::to_string(&ref_kind).map_err(|_| ApiError::InvalidInput)?.trim_matches('"').to_owned(),
        ref_name: ref_name.to_owned(),
        interval_seconds,
        enabled: true,
        next_run_at: (now + chrono::Duration::seconds(interval_seconds)).to_rfc3339(),
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    store.upsert_repository_sync_schedule(&schedule).map_err(ApiError::from)
}

#[utoipa::path(
    delete,
    path = "/specifications/{specification_id}/code-binding",
    params(("specification_id" = String, Path), ("If-Match" = String, Header)),
    responses((status = 204), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 412, body = ErrorResponse), (status = 428, body = ErrorResponse)),
    tag = "code"
)]
async fn delete_specification_code_binding(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let existing = core
        .specification_code_binding(&context, &specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification_code_binding"))?;
    let current = specification_code_binding_response(&core, &context, &existing)?.etag;
    if !headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current))
    {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    core.delete_specification_code_binding(&context, &specification_id)?;
    let audit = audit_event(&context, &headers, "delete", "specification_code_binding", &existing.id);
    inner.store.delete_specification_code_binding(&context, &specification_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_repository_snapshot(
    repository_url: String,
    ref_kind: SpecificationRefKind,
    ref_name: String,
    credential: Option<GitCredential>,
) -> Result<specx_controller::RepositorySnapshotResult, ApiError> {
    let provider_root = std::env::temp_dir().join(format!("specx-repository-controller-{}", uuid::Uuid::new_v4()));
    tokio::task::spawn_blocking(move || {
        let provider = CommandGitProvider::new(&provider_root)?.with_credential(credential);
        let controller = RepositoryController::new(provider);
        let result =
            controller.reconcile(&RepositoryReconcileRequest { repository_url, ref_kind, ref_name, credential: None });
        let _ = std::fs::remove_dir_all(&provider_root);
        result.map_err(ApiError::from)
    })
    .await
    .map_err(|_| ApiError::RepositorySyncFailed)?
}

async fn persist_repository_snapshot(
    state: &ApiState,
    context: &WorkspaceContext,
    repository_id: &str,
    source_ref: &str,
    fetched: &specx_controller::RepositorySnapshotResult,
    fetched_at: &str,
) -> Result<RepositorySnapshot, ApiError> {
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, context)?;
    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let snapshot_input = RepositorySnapshot {
        id: snapshot_id.clone(),
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        repository_id: repository_id.to_owned(),
        source_ref: source_ref.to_owned(),
        resolved_commit: fetched.resolved_commit.clone(),
        manifest_hash: fetched.manifest_hash.clone(),
        content_root_ref: Some(format!("snapshot:{snapshot_id}")),
        fetched_at: fetched_at.to_owned(),
        provider_version: fetched.provider_version.clone(),
        created_at: fetched_at.to_owned(),
    };
    let snapshot = core.record_repository_snapshot(context, snapshot_input)?;
    inner.store.upsert_repository_snapshot(context, &snapshot)?;
    for file in &fetched.files {
        let content =
            state.content_store.put(&file.content, "text/plain; charset=utf-8", fetched_at).map_err(|error| {
                tracing::warn!(error = %error, "repository content could not be stored");
                ApiError::RepositorySyncFailed
            })?;
        let content_ref = serde_json::to_string(&content).map_err(|_| ApiError::RepositorySyncFailed)?;
        inner.store.upsert_repository_snapshot_file(&RepositorySnapshotFile {
            snapshot_id: snapshot.id.clone(),
            path: file.path.clone(),
            content_ref,
            content_hash: file.content_hash.clone(),
            byte_length: file.content.len() as i64,
            media_type: if file.path.to_ascii_lowercase().ends_with(".md")
                || file.path.to_ascii_lowercase().ends_with(".markdown")
            {
                "text/markdown; charset=utf-8".into()
            } else {
                "application/octet-stream".into()
            },
        })?;
    }
    inner.seed_core = None;
    Ok(snapshot)
}

fn read_repository_snapshot_file(
    state: &ApiState,
    inner: &ApiInner,
    snapshot_id: &str,
    path: &str,
) -> Result<String, ApiError> {
    let file = inner
        .store
        .repository_snapshot_files(snapshot_id)?
        .into_iter()
        .find(|file| file.path == path)
        .ok_or(ApiError::NotFound)?;
    let content_ref: specx_types::ContentRef =
        serde_json::from_str(&file.content_ref).map_err(|_| ApiError::RepositorySyncFailed)?;
    let bytes = state.content_store.get(&content_ref).map_err(|error| {
        tracing::warn!(error = %error, snapshot_id, path, "repository snapshot content could not be read");
        ApiError::RepositorySyncFailed
    })?;
    let content = String::from_utf8(bytes).map_err(|_| ApiError::RepositorySyncFailed)?;
    Ok(content.replace("\r\n", "\n").replace('\r', "\n"))
}

#[utoipa::path(
    post,
    path = "/specifications/{specification_id}/sync",
    params(("specification_id" = String, Path)),
    responses(
        (status = 200, description = "Repository-managed specification synchronized", body = SpecificationSyncResponse, headers(("ETag" = String))),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 502, body = ErrorResponse)
    ),
    tag = "code"
)]
async fn sync_specification(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let (expected, binding, repository_id, repository_url, credential) = {
        let mut inner = state.inner.lock().await;
        let core = load_core(&mut inner, &context)?;
        let specification = core
            .list_specifications(&context)
            .into_iter()
            .find(|specification| specification.id == specification_id)
            .cloned()
            .ok_or(CoreError::NotFound("specification"))?;
        let binding = core
            .specification_code_binding(&context, &specification_id)
            .cloned()
            .ok_or(CoreError::NotFound("specification_code_binding"))?;
        if binding.path.is_empty() {
            return Err(CoreError::InvalidSpecificationCodeBinding.into());
        }
        let relation = core
            .service_repository(&context, &specification.service_id)
            .ok_or(CoreError::NotFound("service_repository"))?;
        let repository = core.repository(&context, &relation.repository_id).ok_or(CoreError::NotFound("repository"))?;
        let credential = repository_credential(&inner, &context, &repository.provider)?;
        (specification, binding, repository.id.clone(), repository.canonical_url.clone(), credential)
    };

    let synced_at = Utc::now().to_rfc3339();
    let fetched =
        fetch_repository_snapshot(repository_url, binding.ref_kind, binding.ref_name.clone(), credential).await?;
    let snapshot =
        persist_repository_snapshot(&state, &context, &repository_id, &binding.ref_name, &fetched, &synced_at).await?;
    let content = {
        let inner = state.inner.lock().await;
        read_repository_snapshot_file(&state, &inner, &snapshot.id, &binding.path)?
    };
    let content_hash = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
    let changed = expected.content_hash != content_hash;
    let (specification, snapshot_id) = if changed {
        let mut inner = state.inner.lock().await;
        let mut core = load_core(&mut inner, &context)?;
        let current = core
            .list_specifications(&context)
            .into_iter()
            .find(|specification| specification.id == specification_id)
            .cloned()
            .ok_or(CoreError::NotFound("specification"))?;
        if SpecificationResponse::from(current.clone()).etag != SpecificationResponse::from(expected.clone()).etag {
            return Err(ApiError::PreconditionFailed);
        }
        let updated = core.update_managed_specification_content(&context, &specification_id, content.clone())?;
        let audit = audit_event(&context, &headers, "sync", "specification", &specification_id);
        if !inner.store.update_specification_if_current_with_audit_and_snapshot(
            &context,
            &current,
            &updated,
            Some(&audit),
            Some(&snapshot),
        )? {
            return Err(ApiError::PreconditionFailed);
        }
        inner.seed_core = None;
        let specification = inner
            .store
            .load_core(&context)?
            .list_specifications(&context)
            .into_iter()
            .find(|specification| specification.id == specification_id)
            .cloned()
            .ok_or(CoreError::NotFound("specification"))?;
        (specification, snapshot.id)
    } else {
        (expected.clone(), snapshot.id.clone())
    };

    let specification_response: SpecificationResponse = specification.into();
    let etag = specification_response.etag.clone();
    let response = SpecificationSyncResponse {
        specification: specification_response,
        snapshot_id,
        manifest_hash: fetched.manifest_hash,
        resolved_commit: fetched.resolved_commit,
        content_hash,
        changed,
        synced_at,
        provider_version: fetched.provider_version,
    };
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    post,
    path = "/services/{service_id}/repository",
    params(("service_id" = String, Path), ("Idempotency-Key" = Option<String>, Header)),
    request_body = LinkServiceRepositoryRequest,
    responses((status = 200, body = ServiceRepositoryResponse, headers(("ETag" = String))), (status = 409, body = ErrorResponse), (status = 422, body = ErrorResponse)),
    tag = "code"
)]
async fn link_service_repository(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<LinkServiceRepositoryRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "service-repositories", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "service_repository" {
            return Err(ApiError::IdempotencyConflict);
        }
        let core = load_core(&mut inner, &context)?;
        let relation = core.service_repository(&context, &service_id).ok_or(ApiError::IdempotencyConflict)?;
        if relation.id != record.resource_id {
            return Err(ApiError::IdempotencyConflict);
        }
        let response = service_repository_response(&core, &context, &service_id)?;
        let etag = response.etag.clone();
        return Ok(response_with_etag(response, etag));
    }

    let mut core = load_core(&mut inner, &context)?;
    if !core.list_services(&context).into_iter().any(|service| service.id == service_id) {
        return Err(CoreError::NotFound("service").into());
    }
    let (canonical_url, _, _) = normalize_repository_url(&input.repository_url)?;
    let repository = match core.repository_by_canonical_url(&context, &canonical_url).cloned() {
        Some(repository) => repository,
        None => core.create_repository(&context, uuid::Uuid::new_v4().to_string(), &input.repository_url)?,
    };
    let relation =
        core.link_service_repository(&context, uuid::Uuid::new_v4().to_string(), &service_id, &repository.id)?;
    let audit = audit_event(&context, &headers, "link", "service_repository", &relation.id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "service_repository", &relation.id),
    )?;
    inner.seed_core = None;
    let response = service_repository_response(&core, &context, &service_id)?;
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    delete,
    path = "/services/{service_id}/repository",
    params(("service_id" = String, Path), ("If-Match" = String, Header)),
    responses((status = 204), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 412, body = ErrorResponse), (status = 428, body = ErrorResponse)),
    tag = "code"
)]
async fn unlink_service_repository(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let current = service_repository_response(&core, &context, &service_id)?;
    if !headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current.etag))
    {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    let relation = core.delete_service_repository(&context, &current.id)?;
    let audit = audit_event(&context, &headers, "unlink", "service_repository", &relation.id);
    inner.store.delete_service_repository(&context, &relation.id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/services/{service_id}/code-bindings", params(("service_id" = String, Path)), responses((status = 200, body = ServiceCodeBindingPageResponse)), tag = "code")]
async fn list_service_code_bindings(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
) -> Result<Json<ServiceCodeBindingPageResponse>, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    if !core.list_services(&context).into_iter().any(|service| service.id == service_id) {
        return Err(CoreError::NotFound("service").into());
    }
    let mut items: Vec<_> =
        core.list_service_code_bindings(&context, &service_id).into_iter().cloned().map(Into::into).collect();
    items.sort_by(|left: &ServiceCodeBindingResponse, right: &ServiceCodeBindingResponse| {
        left.role.cmp(&right.role).then_with(|| left.path.cmp(&right.path)).then_with(|| left.id.cmp(&right.id))
    });
    Ok(Json(ServiceCodeBindingPageResponse { items }))
}

#[utoipa::path(post, path = "/services/{service_id}/code-bindings", params(("service_id" = String, Path), ("Idempotency-Key" = Option<String>, Header)), request_body = CreateCodeBindingRequest, responses((status = 200, body = ServiceCodeBindingResponse, headers(("ETag" = String, description = "Strong validator for subsequent updates")))), tag = "code")]
async fn create_code_binding(
    State(state): State<ApiState>,
    Path(service_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<CreateCodeBindingRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "code-bindings", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "code_binding" {
            return Err(ApiError::IdempotencyConflict);
        }
        let response = load_core(&mut inner, &context)?
            .list_code_bindings(&context)
            .into_iter()
            .find(|binding| binding.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        let response: ServiceCodeBindingResponse = response.into();
        let etag = response.etag.clone();
        return Ok(response_with_etag(response, etag));
    }
    let now = Utc::now().to_rfc3339();
    let mut core = load_core(&mut inner, &context)?;
    let binding = core.create_code_binding(
        &context,
        ServiceCodeBinding {
            id: input.id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id,
            source_id: input.source_id,
            role: input.role,
            entity_kind: input.entity_kind,
            path: input.path,
            revision_kind: input.revision_kind,
            revision: input.revision,
            created_at: now.clone(),
            updated_at: now,
        },
    )?;
    let id = binding.id.clone();
    let audit = audit_event(&context, &headers, "create", "code_binding", &id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "code_binding", &id),
    )?;
    inner.seed_core = None;
    let response: ServiceCodeBindingResponse = binding.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(put, path = "/code-bindings/{binding_id}", params(("binding_id" = String, Path), ("If-Match" = String, Header)), request_body = UpdateCodeBindingRequest, responses((status = 200, body = ServiceCodeBindingResponse, headers(("ETag" = String, description = "New strong validator for subsequent updates")))), tag = "code")]
async fn update_code_binding(
    State(state): State<ApiState>,
    Path(binding_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateCodeBindingRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let expected = core
        .list_code_bindings(&context)
        .into_iter()
        .find(|binding| binding.id == binding_id)
        .cloned()
        .ok_or(CoreError::NotFound("code_binding"))?;
    let current = ServiceCodeBindingResponse::from(expected.clone()).etag;
    let matches = headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current));
    if !matches {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    let binding = core.update_code_binding(
        &context,
        &binding_id,
        input.role,
        input.entity_kind,
        input.path,
        input.revision_kind,
        input.revision,
    )?;
    let audit = audit_event(&context, &headers, "update", "code_binding", &binding_id);
    if !inner.store.update_code_binding_if_current_with_audit(&context, &expected, &binding, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let response: ServiceCodeBindingResponse = binding.into();
    let etag = response.etag.clone();
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(delete, path = "/code-bindings/{binding_id}", params(("binding_id" = String, Path), ("If-Match" = String, Header)), responses((status = 204)), tag = "code")]
async fn delete_code_binding(
    State(state): State<ApiState>,
    Path(binding_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let binding = core
        .list_code_bindings(&context)
        .into_iter()
        .find(|binding| binding.id == binding_id)
        .cloned()
        .ok_or(CoreError::NotFound("code_binding"))?;
    let current = ServiceCodeBindingResponse::from(binding).etag;
    let matches = headers
        .get_all(IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| if_match_satisfies(value, &current));
    if !matches {
        return Err(if headers.get(IF_MATCH).is_none() {
            ApiError::PreconditionRequired
        } else {
            ApiError::PreconditionFailed
        });
    }
    core.delete_code_binding(&context, &binding_id)?;
    let audit = audit_event(&context, &headers, "delete", "code_binding", &binding_id);
    inner.store.delete_code_binding(&context, &binding_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSpecificationRequest {
    pub id: Id,
    pub service_id: Id,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub source: Option<CreateSpecificationSourceRequest>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSpecificationSourceRequest {
    pub path: String,
    #[schema(value_type = String)]
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSpecificationRequest {
    pub title: String,
    pub content: String,
}

#[utoipa::path(
    post,
    path = "/specifications",
    params(("Idempotency-Key" = Option<String>, Header, description = "Client supplied key for safe retries")),
    request_body = CreateSpecificationRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (
            status = 200,
            description = "Specification created; returns the current ETag",
            body = SpecificationResponse,
            headers(("ETag" = String, description = "Strong validator for subsequent conditional updates"))
        ),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Service is outside the authorized scope", body = ErrorResponse),
        (status = 409, description = "Idempotency key was already used with a different request", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 404, description = "Service does not exist", body = ErrorResponse),
        (status = 422, description = "Invalid specification", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn create_specification(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<CreateSpecificationRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let descriptor = idempotency_descriptor(&headers, &context, "specifications", &input)?;
    let mut inner = state.inner.lock().await;
    if let Some(record) = check_idempotency(&inner.store, descriptor.as_ref())? {
        if record.resource_type != "specification" {
            return Err(ApiError::IdempotencyConflict);
        }
        let core = load_core(&mut inner, &context)?;
        let specification = core
            .list_specifications(&context)
            .into_iter()
            .find(|specification| specification.id == record.resource_id)
            .cloned()
            .ok_or(ApiError::IdempotencyConflict)?;
        let response = SpecificationResponse::from(specification);
        let etag = specification_etag(&response);
        return Ok(response_with_etag(response, etag));
    }
    let mut core = load_core(&mut inner, &context)?;
    let CreateSpecificationRequest { id, service_id, title, content, source } = input;
    let specification = core.create_markdown_specification(
        &context,
        Specification {
            id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id,
            title,
            kind: SpecificationKind::Markdown,
            content,
            version: 1,
            content_hash: String::new(),
            updated_at: Utc::now().to_rfc3339(),
        },
    )?;
    let specification_id = specification.id.clone();
    if let Some(source) = source {
        let relation = core
            .service_repository(&context, &specification.service_id)
            .ok_or(CoreError::NotFound("service_repository"))?;
        core.set_specification_code_binding(
            &context,
            SpecificationCodeBinding {
                id: uuid::Uuid::new_v4().to_string(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                specification_id: specification_id.clone(),
                service_repository_id: relation.id.clone(),
                path: source.path,
                ref_kind: source.ref_kind,
                ref_name: source.ref_name,
                created_at: specification.updated_at.clone(),
                updated_at: specification.updated_at.clone(),
            },
        )?;
    }
    let audit = audit_event(&context, &headers, "create", "specification", &specification_id);
    inner.store.persist_core_with_audit_and_idempotency(
        &core,
        &context,
        Some(&audit),
        durable_idempotency(descriptor.as_ref(), &context, "specification", &specification_id),
    )?;
    inner.seed_core = None;
    let persisted_core = inner.store.load_core(&context)?;
    let specification = persisted_core
        .list_specifications(&context)
        .into_iter()
        .find(|value| value.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    if let Some(binding) = persisted_core.specification_code_binding(&context, &specification_id)
        && binding.ref_kind == SpecificationRefKind::Branch
        && !binding.path.is_empty()
        && let Some(relation) = persisted_core.service_repository(&context, &specification.service_id)
    {
        upsert_repository_schedule(
            &inner.store,
            &context,
            &relation.repository_id,
            binding.ref_kind,
            &binding.ref_name,
        )?;
    }
    let response = SpecificationResponse::from(specification);
    let etag = specification_etag(&response);
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    put,
    path = "/specifications/{specification_id}",
    params(
        ("specification_id" = String, Path, description = "Stable specification identifier"),
        ("If-Match" = String, Header, description = "Strong ETag from the current specification representation")
    ),
    request_body = UpdateSpecificationRequest,
    responses(
        (status = 400, description = "Malformed request JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Unsupported content type", body = ErrorResponse),
        (
            status = 200,
            description = "Specification updated",
            body = SpecificationResponse,
            headers(("ETag" = String, description = "New strong validator for subsequent conditional updates"))
        ),
        (status = 401, description = "Missing request context", body = ErrorResponse),
        (status = 403, description = "Specification is outside the authorized scope", body = ErrorResponse),
        (status = 429, description = "Actor request rate exceeded", body = ErrorResponse),
        (status = 404, description = "Specification does not exist", body = ErrorResponse),
        (status = 409, description = "Repository-managed specifications cannot be edited", body = ErrorResponse),
        (status = 412, description = "If-Match does not match the current specification", body = ErrorResponse),
        (status = 422, description = "Invalid specification", body = ErrorResponse),
        (status = 428, description = "If-Match is required", body = ErrorResponse),
        (status = 500, description = "Storage failure", body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn update_specification(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<UpdateSpecificationRequest>,
) -> Result<Response, ApiError> {
    let context = request_context(&headers)?;
    let expected_etags: Vec<_> = headers.get_all(IF_MATCH).iter().filter_map(|value| value.to_str().ok()).collect();
    if expected_etags.is_empty() {
        return Err(ApiError::PreconditionRequired);
    }
    let mut inner = state.inner.lock().await;
    let mut core = load_core(&mut inner, &context)?;
    let existing = core
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let existing_response = SpecificationResponse::from(existing.clone());
    let current_etag = specification_etag(&existing_response);
    if !expected_etags.iter().any(|if_match| if_match_satisfies(if_match, &current_etag)) {
        return Err(ApiError::PreconditionFailed);
    }
    let updated = core.update_markdown_specification(&context, &specification_id, input.title, input.content)?;
    let audit = audit_event(&context, &headers, "update", "specification", &specification_id);
    if !inner.store.update_specification_if_current_with_audit(&context, &existing, &updated, Some(&audit))? {
        return Err(ApiError::PreconditionFailed);
    }
    inner.seed_core = None;
    let persisted_core = inner.store.load_core(&context)?;
    let specification = persisted_core
        .list_specifications(&context)
        .into_iter()
        .find(|value| value.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    let response = SpecificationResponse::from(specification);
    let etag = specification_etag(&response);
    Ok(response_with_etag(response, etag))
}

#[utoipa::path(
    delete,
    path = "/specifications/{specification_id}",
    params(
        ("specification_id" = String, Path, description = "Stable specification identifier"),
        ("If-Match" = String, Header, description = "Strong ETag from the current specification representation")
    ),
    responses(
        (status = 400, description = "Malformed path or request", body = ErrorResponse),
        (status = 204, description = "Specification and its immutable revisions deleted"),
        (status = 401, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "Repository-managed specifications cannot be deleted", body = ErrorResponse),
        (status = 412, body = ErrorResponse),
        (status = 428, body = ErrorResponse)
    ),
    tag = "catalog"
)]
async fn delete_specification(
    State(state): State<ApiState>,
    Path(specification_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = request_context(&headers)?;
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context)?;
    let specification = core
        .list_specifications(&context)
        .into_iter()
        .find(|specification| specification.id == specification_id)
        .cloned()
        .ok_or(CoreError::NotFound("specification"))?;
    if core.specification_code_binding(&context, &specification_id).is_some_and(|binding| !binding.path.is_empty()) {
        return Err(CoreError::ManagedSpecification.into());
    }
    let current_etag = SpecificationResponse::from(specification).etag;
    let expected_etags: Vec<_> = headers.get_all(IF_MATCH).iter().filter_map(|value| value.to_str().ok()).collect();
    if expected_etags.is_empty() {
        return Err(ApiError::PreconditionRequired);
    }
    if !expected_etags.iter().any(|if_match| if_match_satisfies(if_match, &current_etag)) {
        return Err(ApiError::PreconditionFailed);
    }
    let audit = audit_event(&context, &headers, "delete", "specification", &specification_id);
    inner.store.delete_specification(&context, &specification_id, &audit)?;
    inner.seed_core = None;
    Ok(StatusCode::NO_CONTENT)
}

fn specification_etag(specification: &SpecificationResponse) -> String {
    let representation = serde_json::to_vec(&(
        &specification.id,
        &specification.tenant_id,
        &specification.workspace_id,
        &specification.service_id,
        &specification.title,
        specification.kind,
        &specification.content,
        specification.version,
        &specification.content_hash,
        &specification.updated_at,
    ))
    .expect("specification ETag payload is serializable");
    let digest = Sha256::digest(representation);
    format!("\"sha256:{digest:x}\"")
}

fn service_etag(service: &ServiceResponse) -> String {
    let representation = serde_json::to_vec(&(
        &service.id,
        &service.tenant_id,
        &service.workspace_id,
        &service.group_id,
        &service.name,
        &service.slug,
        &service.description,
        &service.tag_ids,
        &service.updated_at,
        service.archived,
    ))
    .expect("service ETag payload is serializable");
    let digest = Sha256::digest(representation);
    format!("\"sha256:{digest:x}\"")
}

fn service_repository_etag(repository: &ServiceRepositoryResponse) -> String {
    let representation = serde_json::to_vec(&(
        &repository.id,
        &repository.repository.id,
        &repository.repository.canonical_url,
        &repository.updated_at,
    ))
    .expect("service repository ETag payload is serializable");
    format!("\"sha256:{:x}\"", Sha256::digest(representation))
}

fn specification_code_binding_etag(binding: &SpecificationCodeBindingResponse) -> String {
    let representation = serde_json::to_vec(&(
        &binding.id,
        &binding.specification_id,
        &binding.repository.id,
        &binding.path,
        binding.ref_kind,
        &binding.ref_name,
        &binding.updated_at,
    ))
    .expect("specification code binding ETag payload is serializable");
    format!("\"sha256:{:x}\"", Sha256::digest(representation))
}

fn code_source_etag(source: &CodeSourceResponse) -> String {
    let representation = serde_json::to_vec(&(
        &source.id,
        &source.tenant_id,
        &source.workspace_id,
        source.source_type,
        &source.provider,
        &source.display_name,
        &source.canonical_key,
        &source.root_locator,
        source.execution_scope,
        &source.updated_at,
        source.archived,
    ))
    .expect("code source ETag payload is serializable");
    format!("\"sha256:{:x}\"", Sha256::digest(representation))
}

fn code_binding_etag(binding: &ServiceCodeBindingResponse) -> String {
    let representation = serde_json::to_vec(&(
        &binding.id,
        &binding.tenant_id,
        &binding.workspace_id,
        &binding.service_id,
        &binding.source_id,
        binding.role,
        binding.entity_kind,
        &binding.path,
        binding.revision_kind,
        &binding.revision,
        &binding.updated_at,
    ))
    .expect("code binding ETag payload is serializable");
    format!("\"sha256:{:x}\"", Sha256::digest(representation))
}

fn if_match_satisfies(if_match: &str, current_etag: &str) -> bool {
    if_match.split(',').any(|candidate| candidate.trim() == "*" || candidate.trim() == current_etag)
}

fn response_with_etag<T: Serialize>(value: T, etag: String) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(ETAG, HeaderValue::from_str(&etag).expect("generated ETag is valid"));
    response
}

#[derive(OpenApi)]
#[openapi(
    info(title = "specX API", version = "1.0.0", description = "Service catalog and specification API."),
    servers((url = "/api/specx")),
    paths(health, ready, metrics, get_access, get_catalog, list_workspaces, create_workspace, update_workspace, delete_workspace, list_members, set_member_role, revoke_member, list_invitations, create_invitation, revoke_invitation, accept_invitation, list_groups, update_group, reorder_groups, delete_group, list_tags, update_tag, delete_tag, list_services, list_specifications, list_specification_revisions, get_specification_revision, restore_specification_revision, get_specification_code_binding, set_specification_code_binding, delete_specification_code_binding, get_specification_sync_status, sync_specification, discover_repository_files, enqueue_repository_reconcile, get_controller_task, repository_webhook, list_secrets, create_secret, update_secret, delete_secret, list_integrations, create_integration, create_group, create_tag, create_service, update_service, set_service_archived, delete_service, list_code_sources, create_code_source, update_code_source, delete_code_source, get_service_repository, link_service_repository, unlink_service_repository, list_service_code_bindings, create_code_binding, update_code_binding, delete_code_binding, create_specification, update_specification, delete_specification, list_backups, create_backup, restore_backup),
    components(schemas(
        HealthResponse,
        ReadyResponse,
        CatalogResponse,
        GroupPageResponse,
        TagPageResponse,
        SpecificationPageResponse,
        SpecificationSyncResponse,
        RepositoryDiscoveryQuery,
        RepositoryFileResponse,
        RepositoryDiscoveryResponse,
        ControllerTaskResponse,
        RepositoryWebhookRequest,
        CreateSecretRequest,
        SecretResponse,
        SecretPageResponse,
        UpdateSecretRequest,
        CreateIntegrationRequest,
        IntegrationResponse,
        IntegrationPageResponse,
        SpecificationRevisionPageResponse,
        SpecificationRevisionResponse,
        ServicePageResponse,
        WorkspaceRole,
        WorkspaceResponse,
        WorkspaceListResponse,
        CreateWorkspaceRequest,
        UpdateWorkspaceRequest,
        WorkspaceMemberResponse,
        WorkspaceMemberPageResponse,
        SetMemberRoleRequest,
        CreateInvitationRequest,
        AcceptInvitationRequest,
        InvitationResponse,
        CreatedInvitationResponse,
        AcceptedInvitationResponse,
        ServiceGroupResponse,
        TagResponse,
        ServiceResponse,
        CodeSourceResponse,
        CodeSourcePageResponse,
        CreateCodeSourceRequest,
        UpdateCodeSourceRequest,
        ServiceCodeBindingResponse,
        ServiceCodeBindingPageResponse,
        CreateCodeBindingRequest,
        UpdateCodeBindingRequest,
        RepositorySummaryResponse,
        ServiceRepositoryResponse,
        LinkServiceRepositoryRequest,
        SpecificationCodeBindingResponse,
        SetSpecificationCodeBindingRequest,
        SpecificationKindResponse,
        SpecificationResponse,
        CreateGroupRequest,
        UpdateGroupRequest,
        GroupOrderItem,
        ReorderGroupsRequest,
        CreateTagRequest,
        UpdateTagRequest,
        CreateServiceRequest,
        UpdateServiceRequest,
        SetServiceArchivedRequest,
        CreateSpecificationRequest,
        UpdateSpecificationRequest,
        BackupResponse,
        ErrorResponse
    )),
    tags(
        (name = "system", description = "Service health"),
        (name = "catalog", description = "Workspace service catalog and specifications"),
        (name = "code", description = "Code sources and service code bindings"),
        (name = "workspaces", description = "Workspace metadata and membership access"),
        (name = "members", description = "Workspace member roles and access")
        ,(name = "backups", description = "Server-owned specX SQLite snapshots")
    )
)]
pub struct ApiDoc;

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
