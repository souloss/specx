//! Native specX host.
//!
//! The desktop process is deliberately a thin transport adapter. It owns the
//! local process lifetime and persistence configuration, then dispatches catalog requests
//! through the same `specx-api` router used by the Web host. Keeping the router
//! in the call path means authorization, idempotency, audit events, optimistic
//! concurrency, and persistence semantics cannot drift between hosts.

mod release;

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use specx_api::{ApiState, errors::ErrorKind, router};
use specx_platform::{DatabaseUrl, parse_database_url};
use tauri::{Manager, State};
use tokio::sync::Mutex;
use tower::ServiceExt;

#[derive(Clone)]
struct DesktopState {
    api: Arc<Mutex<Option<ApiState>>>,
    database: DatabaseUrl,
    backup_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApiRequest {
    method: String,
    path: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApiResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: serde_json::Value,
}

/// Execute one frontend request against the shared in-process API router.
#[tauri::command]
async fn specx_api_request(
    state: State<'_, DesktopState>,
    request: DesktopApiRequest,
) -> Result<DesktopApiResponse, ()> {
    // Tauri requires Result for async commands borrowing State. API failures
    // still travel as successful IPC responses with their HTTP-style envelope.
    let api = {
        let mut current = state.api.lock().await;
        if current.is_none() {
            match open_api(&state.database, &state.backup_dir).await {
                Ok(api) => *current = Some(api),
                Err(error) => {
                    eprintln!("specX storage migration is not ready: {error}");
                    return Ok(desktop_error_response(ErrorKind::NotReady));
                }
            }
        }
        current.clone().expect("storage state is initialized")
    };
    Ok(dispatch_api_request(&api, request).await.unwrap_or_else(desktop_error_response))
}

async fn open_api(database: &DatabaseUrl, backup_dir: &Path) -> Result<ApiState, String> {
    match database {
        DatabaseUrl::Sqlite(path) => {
            ApiState::open_with_backup_root(path, backup_dir).map_err(|error| error.to_string())
        }
        DatabaseUrl::Postgres(url) => {
            let pool_size =
                env::var("SPECX_DATABASE_MAX_POOL_SIZE").ok().and_then(|value| value.parse().ok()).unwrap_or(16);
            ApiState::open_postgres_with_backup_root(url, pool_size, backup_dir)
                .await
                .map_err(|error| error.to_string())
        }
    }
}

fn desktop_error_response(error: ErrorKind) -> DesktopApiResponse {
    DesktopApiResponse {
        status: error.status().as_u16(),
        headers: BTreeMap::new(),
        body: serde_json::to_value(error.body()).expect("catalog error is serializable"),
    }
}

fn native_window_decorations_override(target_os: &str) -> Option<bool> {
    match target_os {
        "windows" | "linux" => Some(false),
        _ => None,
    }
}

async fn dispatch_api_request(api: &ApiState, request: DesktopApiRequest) -> Result<DesktopApiResponse, ErrorKind> {
    if request.path == "/api/auth/check" || request.path == "/api/auth/login" {
        return Ok(DesktopApiResponse {
            status: StatusCode::OK.as_u16(),
            headers: BTreeMap::new(),
            body: serde_json::json!({ "authenticated": true, "setup_required": false }),
        });
    }
    if request.path == "/api/auth/logout" {
        return Ok(DesktopApiResponse {
            status: StatusCode::NO_CONTENT.as_u16(),
            headers: BTreeMap::new(),
            body: serde_json::Value::Null,
        });
    }

    let path = request.path.strip_prefix("/api/specx").ok_or(ErrorKind::NotFound)?;
    let path = if path.is_empty() { "/" } else { path };
    let method: axum::http::Method = request.method.parse().map_err(|_| ErrorKind::InvalidRequest)?;
    let workspace_id = request.headers.get("x-specx-workspace-id").cloned().unwrap_or_else(|| "default".to_owned());
    let mut builder = Request::builder().method(method).uri(path);
    for (name, value) in request.headers {
        let name = HeaderName::try_from(name).map_err(|_| ErrorKind::InvalidRequest)?;
        if matches!(
            name.as_str(),
            "x-specx-tenant-id"
                | "x-specx-actor-id"
                | "x-specx-workspace-id"
                | "host"
                | "origin"
                | "cookie"
                | "content-length"
                | "transfer-encoding"
                | "sec-fetch-site"
        ) {
            continue;
        }
        let value = HeaderValue::try_from(value).map_err(|_| ErrorKind::InvalidRequest)?;
        builder = builder.header(name, value);
    }
    // The local shell supplies a stable native identity. The API still runs its
    // normal workspace membership bootstrap and authorization checks.
    builder = builder.header("x-specx-tenant-id", "specx");
    builder = builder.header("x-specx-actor-id", "specx-desktop-local");
    builder = builder.header("x-specx-workspace-id", workspace_id);
    let body = request.body.unwrap_or_default();
    if !body.is_empty() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder.body(Body::from(body)).map_err(|_| ErrorKind::InvalidRequest)?;
    let response = router::<()>(api.clone()).oneshot(request).await.map_err(|_| ErrorKind::Internal)?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.to_string(), value.to_owned())))
        .collect();
    let bytes = response.into_body().collect().await.map_err(|_| ErrorKind::Internal)?.to_bytes();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    Ok(DesktopApiResponse { status, headers, body })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if let Some(decorations) = native_window_decorations_override(std::env::consts::OS)
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.set_decorations(decorations);
            }
            let default_data_dir = app.path().app_data_dir()?;
            let data_dir = env::var_os("SPECX_DATA_DIR").map(PathBuf::from).unwrap_or(default_data_dir);
            let backup_dir =
                env::var_os("SPECX_BACKUP_ROOT").map(PathBuf::from).unwrap_or_else(|| data_dir.join("backups"));
            fs::create_dir_all(&data_dir)?;
            fs::create_dir_all(&backup_dir)?;
            let database = parse_database_url(env::var("SPECX_DATABASE_URL").ok().as_deref(), &data_dir)
                .map_err(std::io::Error::other)?;
            if let DatabaseUrl::Sqlite(path) = &database
                && let Some(parent) = path.parent()
            {
                fs::create_dir_all(parent)?;
            }
            let initial_api = tauri::async_runtime::block_on(open_api(&database, &backup_dir));
            if let Err(error) = &initial_api {
                eprintln!("specX storage migration will be retried after startup: {error}");
            }
            app.manage(DesktopState { api: Arc::new(Mutex::new(initial_api.ok())), database, backup_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            specx_api_request,
            release::get_app_release_info,
            release::fetch_app_changelog
        ])
        .run(tauri::generate_context!())
        .expect("error while running specX desktop application");
}

#[cfg(test)]
mod tests {
    use super::{DesktopApiRequest, desktop_error_response, dispatch_api_request, native_window_decorations_override};
    use specx_api::ApiState;

    #[test]
    fn native_window_decorations_match_platform_titlebar_strategy() {
        assert_eq!(native_window_decorations_override("windows"), Some(false));
        assert_eq!(native_window_decorations_override("linux"), Some(false));
        assert_eq!(native_window_decorations_override("macos"), None);
    }

    #[test]
    fn desktop_bridge_uses_the_shared_router_and_bootstraps_its_workspace() {
        tauri::async_runtime::block_on(async {
            let directory = tempfile::tempdir().expect("temporary data directory");
            let api = ApiState::open(directory.path().join("specx.sqlite")).expect("open test API");
            let response = dispatch_api_request(
                &api,
                DesktopApiRequest {
                    method: "GET".into(),
                    path: "/api/specx/workspaces".into(),
                    headers: [
                        ("x-specx-tenant-id".to_owned(), "untrusted-tenant".to_owned()),
                        ("x-specx-actor-id".to_owned(), "untrusted-actor".to_owned()),
                    ]
                    .into(),
                    body: None,
                },
            )
            .await
            .expect("dispatch workspaces request");

            assert_eq!(response.status, 200);
            assert_eq!(response.body["items"][0]["id"], "default");
            assert_eq!(response.body["items"][0]["role"], "owner");
        });
    }
    #[test]
    fn desktop_bridge_preserves_api_errors_and_normalizes_invalid_requests() {
        tauri::async_runtime::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let api = ApiState::open(directory.path().join("specx.sqlite")).unwrap();
            for (path, method, body, expected_status, key) in [
                ("/api/specx/groups", "POST", Some(r#"{"id":"g","name":""}"#), 422, "emptyName"),
                ("/api/specx/groups", "POST", Some("{secret"), 400, "invalidRequest"),
                ("/api/unsupported", "GET", None, 404, "notFound"),
                ("/api/specx/catalog", "invalid method", None, 400, "invalidRequest"),
            ] {
                let result = dispatch_api_request(
                    &api,
                    DesktopApiRequest {
                        method: method.into(),
                        path: path.into(),
                        headers: Default::default(),
                        body: body.map(str::to_owned),
                    },
                )
                .await;
                let response = result.unwrap_or_else(desktop_error_response);
                assert_eq!(response.status, expected_status);
                assert_eq!(response.body["version"], 1);
                assert_eq!(response.body["messageKey"], format!("apiError.{key}"));
                assert_eq!(response.body["messageParams"], serde_json::json!({}));
                assert!(!response.body.to_string().contains("secret"));
            }
        });
    }
}
