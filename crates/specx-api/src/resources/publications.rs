use super::*;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenApiPublicationResponse {
    id: String,
    version: i64,
    title: String,
    description: String,
    theme: String,
    show_download: bool,
    allow_try_it: bool,
    created_at: String,
}

impl From<OpenApiPublication> for OpenApiPublicationResponse {
    fn from(publication: OpenApiPublication) -> Self {
        Self {
            id: publication.id,
            version: publication.version,
            title: publication.title,
            description: publication.description,
            theme: publication.theme,
            show_download: publication.show_download,
            allow_try_it: publication.allow_try_it,
            created_at: publication.created_at,
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenApiPublicationPage {
    items: Vec<OpenApiPublicationResponse>,
    next_offset: Option<usize>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenApiPublicationPreview {
    content: String,
    digest: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreateOpenApiPublication {
    version: i64,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_publication_theme")]
    theme: String,
    #[serde(default = "default_show_download")]
    show_download: bool,
    #[serde(default = "default_allow_try_it")]
    allow_try_it: bool,
    digest: String,
    #[serde(default)]
    environments: Vec<PublicOpenApiEnvironment>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PublicOpenApiEnvironment {
    name: String,
    base_url: String,
    variables: Vec<PublicOpenApiVariable>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicOpenApiVariable {
    key: String,
    value: String,
}

fn default_publication_theme() -> String {
    "light".to_owned()
}

fn default_show_download() -> bool {
    true
}

fn default_allow_try_it() -> bool {
    true
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublicOpenApiDocument {
    publication: OpenApiPublicationResponse,
    content: String,
}

impl StoreBackend {
    fn openapi_publications(
        &self,
        context: &WorkspaceContext,
        id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OpenApiPublication>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.openapi_publications(context, id, limit, offset),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let (context, id) = (context.clone(), id.to_owned());
                run_postgres_future(store.clone(), move |store| async move {
                    store.openapi_publications(&context, &id, limit, offset).await
                })
            }
        }
    }

    fn create_openapi_publication(
        &self,
        context: &WorkspaceContext,
        id: &str,
        publication: &OpenApiPublication,
        content: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.create_openapi_publication(context, id, publication, content, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let (context, id, publication, content) =
                    (context.clone(), id.to_owned(), publication.clone(), content.to_owned());
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.create_openapi_publication(&context, &id, &publication, &content, &audit).await
                })
            }
        }
    }

    fn revoke_openapi_publication(
        &self,
        context: &WorkspaceContext,
        specification: &str,
        id: &str,
        audit: &AuditEvent,
    ) -> Result<bool, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.revoke_openapi_publication(context, specification, id, audit),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let (context, specification, id) = (context.clone(), specification.to_owned(), id.to_owned());
                let audit = audit.clone();
                run_postgres_future(store.clone(), move |store| async move {
                    store.revoke_openapi_publication(&context, &specification, &id, &audit).await
                })
            }
        }
    }

    fn public_openapi_document(&self, id: &str) -> Result<Option<(OpenApiPublication, String)>, PersistenceError> {
        match self {
            Self::Sqlite(store) => store.public_openapi_document(id),
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => {
                let id = id.to_owned();
                run_postgres_future(store.clone(), move |store| async move { store.public_openapi_document(&id).await })
            }
        }
    }
}

fn require_publication_manager(
    inner: &mut ApiInner,
    headers: &HeaderMap,
    id: &str,
) -> Result<WorkspaceContext, ApiError> {
    let context = request_context(headers)?;
    let role = inner.store.workspace_member_role(&context, &context.actor_id)?.ok_or(ApiError::Forbidden)?;
    if !can_manage_members(&role) {
        return Err(ApiError::Forbidden);
    }
    if !load_core(inner, &context)?
        .list_specifications(&context)
        .iter()
        .any(|spec| spec.id == id && spec.kind == SpecificationKind::OpenApi)
    {
        return Err(ApiError::NotFound);
    }
    Ok(context)
}

fn publication_snapshot(
    store: &StoreBackend,
    context: &WorkspaceContext,
    id: &str,
    version: i64,
    title: &str,
    description: &str,
    environments: &[PublicOpenApiEnvironment],
) -> Result<OpenApiPublicationPreview, ApiError> {
    let revision = store.specification_revision(context, id, version)?.ok_or(ApiError::NotFound)?;
    let mut document = specx_formats::parse_openapi_value(&revision.content).map_err(|_| ApiError::InvalidInput)?;
    // A published document does not inherit workspace settings or advertise
    // deployment addresses. Preserve contract examples and model properties.
    if let Some(root) = document.as_object_mut() {
        root.remove("servers");
        root.remove("x-specx-public-environments");
        for name in ["paths", "webhooks"] {
            if let Some(paths) = root.get_mut(name).and_then(serde_json::Value::as_object_mut) {
                paths.values_mut().for_each(remove_path_servers);
            }
        }
        if let Some(paths) = root
            .get_mut("components")
            .and_then(|component| component.get_mut("pathItems"))
            .and_then(serde_json::Value::as_object_mut)
        {
            paths.values_mut().for_each(remove_path_servers);
        }
        if let Some(callbacks) = root
            .get_mut("components")
            .and_then(|component| component.get_mut("callbacks"))
            .and_then(serde_json::Value::as_object_mut)
        {
            callbacks.values_mut().for_each(remove_callback_servers);
        }
    }
    // Only explicitly reviewed public settings may be added to the snapshot.
    if !environments.is_empty() {
        let servers = public_environment_servers(environments)?;
        document["servers"] = serde_json::Value::Array(servers);
        document["x-specx-public-environments"] =
            serde_json::to_value(environments).map_err(|_| ApiError::InvalidInput)?;
    }
    for secret in store.list_secrets(&context.tenant_id, &context.workspace_id)? {
        let value = decrypt_secret(&secret.encrypted_value)?;
        if !value.is_empty()
            && (title.contains(&value) || description.contains(&value) || contains_secret(&document, &value))
        {
            return Err(ApiError::InvalidInput);
        }
    }
    let content = serde_json::to_string(&document).map_err(|_| ApiError::InvalidInput)?;
    if content.len() > 5 * 1024 * 1024 {
        return Err(ApiError::InvalidInput);
    }
    let digest = format!("{:x}", Sha256::digest(content.as_bytes()));
    Ok(OpenApiPublicationPreview { content, digest })
}

fn public_environment_servers(environments: &[PublicOpenApiEnvironment]) -> Result<Vec<serde_json::Value>, ApiError> {
    if environments.len() > 20 {
        return Err(ApiError::InvalidInput);
    }
    let mut names = std::collections::HashSet::new();
    let mut servers = Vec::new();
    for environment in environments {
        if environment.name.trim().is_empty()
            || environment.name.len() > 80
            || environment.name.chars().any(char::is_control)
            || !names.insert(environment.name.trim().to_lowercase())
            || environment.base_url.len() > 2_000
            || environment.variables.len() > 100
        {
            return Err(ApiError::InvalidInput);
        }
        let mut variables = std::collections::HashMap::new();
        for variable in &environment.variables {
            if variable.key.is_empty()
                || variable.key.len() > 120
                || !variable.key.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
                || variable.value.len() > 64 * 1024
                || variable.value.chars().any(char::is_control)
                || variables.insert(variable.key.as_str(), variable.value.as_str()).is_some()
            {
                return Err(ApiError::InvalidInput);
            }
        }
        let mut remaining = environment.base_url.as_str();
        let mut url = String::new();
        while let Some((start, closing)) = remaining
            .find("{{")
            .map(|start| (start, "}}"))
            .into_iter()
            .chain(remaining.find("<<").map(|start| (start, ">>")))
            .min_by_key(|(start, _)| *start)
        {
            url.push_str(&remaining[..start]);
            remaining = &remaining[start + 2..];
            let end = remaining.find(closing).ok_or(ApiError::InvalidInput)?;
            let value = variables.get(remaining[..end].trim()).ok_or(ApiError::InvalidInput)?;
            url.push_str(value);
            remaining = &remaining[end + 2..];
        }
        url.push_str(remaining);
        let uri = url.parse::<http::Uri>().map_err(|_| ApiError::InvalidInput)?;
        if !matches!(uri.scheme_str(), Some("http" | "https"))
            || uri.host().is_none_or(str::is_empty)
            || uri.authority().is_some_and(|authority| authority.as_str().contains('@'))
            || uri.query().is_some()
            || url
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '{' | '}' | '<' | '>' | '#'))
        {
            return Err(ApiError::InvalidInput);
        }
        servers.push(serde_json::json!({"url":url,"description":environment.name}));
    }
    Ok(servers)
}

fn remove_callback_servers(callback: &mut serde_json::Value) {
    if let Some(paths) = callback.as_object_mut() {
        paths.values_mut().for_each(remove_path_servers);
    }
}

fn remove_path_servers(path: &mut serde_json::Value) {
    let Some(item) = path.as_object_mut() else { return };
    item.remove("servers");
    for method in ["get", "post", "put", "patch", "delete", "head", "options", "trace"] {
        if let Some(operation) = item.get_mut(method).and_then(serde_json::Value::as_object_mut) {
            operation.remove("servers");
            if let Some(callbacks) = operation.get_mut("callbacks").and_then(serde_json::Value::as_object_mut) {
                callbacks.values_mut().for_each(remove_callback_servers);
            }
        }
    }
}

fn contains_secret(value: &serde_json::Value, secret: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(secret),
        serde_json::Value::Number(number) => number.to_string() == secret,
        serde_json::Value::Object(items) => {
            items.iter().any(|(key, value)| key.contains(secret) || contains_secret(value, secret))
        }
        serde_json::Value::Array(items) => items.iter().any(|value| contains_secret(value, secret)),
        _ => false,
    }
}

#[utoipa::path(
    get, path = "/specifications/{specification_id}/publications",
    params(("specification_id" = String, Path), OffsetListQuery),
    responses((status = 200, body = OpenApiPublicationPage), (status = 400, body = ErrorResponse), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "openapi"
)]
pub(super) async fn list_openapi_publications(
    State(state): State<ApiState>,
    Path(id): Path<Id>,
    headers: HeaderMap,
    Query(query): Query<OffsetListQuery>,
) -> Result<Json<OpenApiPublicationPage>, ApiError> {
    let (limit, offset, _) = page_bounds(&query, 0)?;
    let offset_i64 = i64::try_from(offset).map_err(|_| ApiError::InvalidQuery)?;
    let mut inner = state.inner.lock().await;
    let context = require_publication_manager(&mut inner, &headers, &id)?;
    let rows = inner.store.openapi_publications(&context, &id, limit as i64 + 1, offset_i64)?;
    let next_offset = (rows.len() > limit).then_some(offset.saturating_add(limit));
    Ok(Json(OpenApiPublicationPage { items: rows.into_iter().take(limit).map(Into::into).collect(), next_offset }))
}

#[utoipa::path(
    get, path = "/specifications/{specification_id}/publication-preview/{version}",
    params(("specification_id" = String, Path), ("version" = i64, Path)),
    responses((status = 200, body = OpenApiPublicationPreview), (status = 400, body = ErrorResponse), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 422, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "openapi"
)]
pub(super) async fn preview_openapi_publication(
    State(state): State<ApiState>,
    Path((id, version)): Path<(Id, i64)>,
    headers: HeaderMap,
) -> Result<Json<OpenApiPublicationPreview>, ApiError> {
    let mut inner = state.inner.lock().await;
    let context = require_publication_manager(&mut inner, &headers, &id)?;
    Ok(Json(publication_snapshot(&inner.store, &context, &id, version, "", "", &[])?))
}

#[utoipa::path(
    post, path = "/specifications/{specification_id}/publications",
    params(("specification_id" = String, Path)), request_body = CreateOpenApiPublication,
    responses((status = 201, body = OpenApiPublicationResponse), (status = 400, body = ErrorResponse), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 412, body = ErrorResponse), (status = 415, body = ErrorResponse), (status = 422, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "openapi"
)]
pub(super) async fn create_openapi_publication(
    State(state): State<ApiState>,
    Path(id): Path<Id>,
    headers: HeaderMap,
    Json(input): Json<CreateOpenApiPublication>,
) -> Result<(StatusCode, Json<OpenApiPublicationResponse>), ApiError> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 || title.chars().any(char::is_control) {
        return Err(ApiError::InvalidInput);
    }
    let description = input.description.trim();
    if description.chars().count() > 500
        || description.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || !matches!(input.theme.as_str(), "light" | "dark")
    {
        return Err(ApiError::InvalidInput);
    }
    let mut inner = state.inner.lock().await;
    let context = require_publication_manager(&mut inner, &headers, &id)?;
    let snapshot = publication_snapshot(&inner.store, &context, &id, input.version, title, description, &[])?;
    if snapshot.digest != input.digest {
        return Err(ApiError::PreconditionFailed);
    }
    let snapshot =
        publication_snapshot(&inner.store, &context, &id, input.version, title, description, &input.environments)?;
    let publication = OpenApiPublication {
        id: uuid::Uuid::new_v4().simple().to_string(),
        version: input.version,
        title: title.into(),
        description: description.into(),
        theme: input.theme,
        show_download: input.show_download,
        allow_try_it: input.allow_try_it,
        created_at: Utc::now().to_rfc3339(),
    };
    let audit = audit_event(&context, &headers, "publish", "openapi_document", &id);
    if !inner.store.create_openapi_publication(&context, &id, &publication, &snapshot.content, &audit)? {
        return Err(ApiError::Conflict);
    }
    Ok((StatusCode::CREATED, Json(publication.into())))
}

#[utoipa::path(
    delete, path = "/specifications/{specification_id}/publications/{publication_id}",
    params(("specification_id" = String, Path), ("publication_id" = String, Path)),
    responses((status = 204, description = "Publication revoked"), (status = 401, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 429, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "openapi"
)]
pub(super) async fn revoke_openapi_publication(
    State(state): State<ApiState>,
    Path((specification, id)): Path<(Id, Id)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let mut inner = state.inner.lock().await;
    let context = require_publication_manager(&mut inner, &headers, &specification)?;
    let audit = audit_event(&context, &headers, "revoke", "openapi_document", &specification);
    if !inner.store.revoke_openapi_publication(&context, &specification, &id, &audit)? {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/public/openapi/{publication_id}", params(("publication_id" = String, Path)),
    responses((status = 200, body = PublicOpenApiDocument), (status = 404, body = ErrorResponse), (status = 500, body = ErrorResponse)),
    tag = "openapi"
)]
pub(super) async fn get_public_openapi_document(
    State(state): State<ApiState>,
    Path(id): Path<Id>,
) -> Result<Json<PublicOpenApiDocument>, ApiError> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ApiError::NotFound);
    }
    let inner = state.inner.lock().await;
    let (publication, content) = inner.store.public_openapi_document(&id)?.ok_or(ApiError::NotFound)?;
    Ok(Json(PublicOpenApiDocument { publication: publication.into(), content }))
}
