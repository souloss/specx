use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::SET_COOKIE},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specx_api::errors::{ErrorKind, normalize_error_response};

#[derive(Clone)]
pub(crate) struct HostState {
    password: Arc<String>,
    session_secret: Arc<String>,
}

impl HostState {
    pub(crate) fn new(password: String, session_secret: String) -> Self {
        Self { password: Arc::new(password), session_secret: Arc::new(session_secret) }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct AuthStatus {
    authenticated: bool,
    setup_required: bool,
}

#[derive(Clone)]
pub(crate) struct Authenticated;

pub(crate) fn router() -> Router<HostState> {
    Router::new().nest(
        "/api/auth",
        Router::new()
            .route("/check", get(auth_check))
            .route("/login", post(auth_login))
            .route("/logout", post(auth_logout))
            .fallback(|| async { ErrorKind::NotFound })
            .layer(middleware::from_fn(normalize_error_response)),
    )
}

pub(crate) async fn auth_check(request: Request<axum::body::Body>) -> Response {
    let authenticated = request.extensions().get::<Authenticated>().is_some();
    Json(AuthStatus { authenticated, setup_required: false }).into_response()
}

pub(crate) async fn auth_login(State(host): State<HostState>, Json(input): Json<LoginRequest>) -> Response {
    if input.password != *host.password {
        return ErrorKind::InvalidCredentials.into_response();
    }
    let token = session_token(&host, &input.password);
    let mut response = Json(AuthStatus { authenticated: true, setup_required: false }).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!("specx_session={token}; Path=/; HttpOnly; SameSite=Lax"))
            .expect("valid session cookie"),
    );
    response
}

pub(crate) async fn auth_logout() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, HeaderValue::from_static("specx_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"));
    response
}

pub(crate) async fn request_context(
    State(host): State<HostState>,
    mut request: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let token = cookie(request.headers().get("cookie").and_then(|value| value.to_str().ok()), "specx_session");
    if let Some(token) = token.filter(|value| value == &session_token(&host, &*host.password)) {
        let actor = format!("specx-session-{}", &token[..16]);
        request.extensions_mut().insert(Authenticated);
        if request.uri().path().starts_with("/api/specx/") {
            let workspace = request
                .headers()
                .get("x-specx-workspace-id")
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("default");
            let workspace = HeaderValue::from_str(workspace).expect("validated workspace header");
            request.headers_mut().insert("x-specx-tenant-id", HeaderValue::from_static("specx"));
            request.headers_mut().insert("x-specx-workspace-id", workspace);
            request
                .headers_mut()
                .insert("x-specx-actor-id", HeaderValue::from_str(&actor).expect("valid actor header"));
        }
    }
    next.run(request).await
}

fn session_token(host: &HostState, password: impl AsRef<str>) -> String {
    let mut digest = Sha256::new();
    digest.update(host.session_secret.as_bytes());
    digest.update(b"\0");
    digest.update(password.as_ref().as_bytes());
    hex::encode(digest.finalize())
}

fn cookie(header: Option<&str>, name: &str) -> Option<String> {
    header?.split(';').map(str::trim).find_map(|part| part.strip_prefix(&format!("{name}=")).map(ToOwned::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn login_errors_and_rejections_share_the_api_catalog() {
        let app = router().with_state(HostState::new("test-password".into(), "test-secret".into()));
        for (input, content_type, status, expected) in [
            (r#"{"password":"wrong"}"#, "application/json", 401, ErrorKind::InvalidCredentials),
            ("{private SQL", "application/json", 400, ErrorKind::InvalidRequest),
            (r#"{}"#, "application/json", 422, ErrorKind::InvalidInput),
            ("private SQL", "text/plain", 415, ErrorKind::UnsupportedMediaType),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/api/auth/login")
                        .header("content-type", content_type)
                        .body(Body::from(input))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            assert!(response.headers().get(SET_COOKIE).is_none());
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body, serde_json::to_value(expected.body()).unwrap());
        }
    }
    #[tokio::test]
    async fn api_fallbacks_stay_json_when_the_host_has_a_spa_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let api = specx_api::ApiState::open(directory.path().join("specx.sqlite")).unwrap();
        let host = HostState::new("test-password".into(), "test-secret".into());
        let token = session_token(&host, "test-password");
        let app = Router::new()
            .merge(router())
            .nest("/api/specx", specx_api::router(api))
            .fallback(|| async { axum::response::Html("<html>SPA fallback</html>") })
            .layer(middleware::from_fn_with_state(host.clone(), request_context))
            .with_state(host);
        for path in ["/api/auth/unknown", "/api/specx/unknown"] {
            let response = app
                .clone()
                .oneshot(
                    Request::get(path).header("cookie", format!("specx_session={token}")).body(Body::empty()).unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body, serde_json::to_value(ErrorKind::NotFound.body()).unwrap());
        }
    }
}
