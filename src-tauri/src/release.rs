//! Desktop release metadata and changelog fetching. Installation and signature
//! verification are owned by the official Tauri updater, not the domain/API.

use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseInfo {
    version: String,
    updater_configured: bool,
}

#[tauri::command]
pub fn get_app_release_info(app: AppHandle) -> AppReleaseInfo {
    let updater = app.config().plugins.0.get("updater");
    let has_key = updater
        .and_then(|value| value.get("pubkey"))
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    AppReleaseInfo {
        version: app.package_info().version.to_string(),
        updater_configured: has_key && changelog_base_url(&app).is_some(),
    }
}

// Only the build's configured HTTPS update origin can be used. The webview
// cannot ask this command to fetch arbitrary URLs or a user's credentials.
fn changelog_base_url(app: &AppHandle) -> Option<String> {
    let endpoints = app.config().plugins.0.get("updater")?.get("endpoints")?.as_array()?;
    let endpoint = endpoints.first()?.as_str()?;
    let url = reqwest::Url::parse(endpoint).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    endpoint.strip_suffix("latest.json").map(str::to_owned)
}

#[tauri::command]
pub async fn fetch_app_changelog(app: AppHandle, locale: String) -> Result<serde_json::Value, String> {
    let locale = match locale.as_str() {
        "zh-CN" => "zh-CN",
        "en" => "en",
        _ => return Err("unsupported_locale".into()),
    };
    let base = changelog_base_url(&app).ok_or("updater_not_configured")?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("specX/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "changelog_unavailable")?;
    let mut response = client
        .get(format!("{base}releases-{locale}.json"))
        .send()
        .await
        .map_err(|_| "changelog_unavailable")?
        .error_for_status()
        .map_err(|_| "changelog_unavailable")?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "changelog_unavailable")? {
        if body.len().saturating_add(chunk.len()) > 2 * 1024 * 1024 {
            return Err("changelog_too_large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "invalid_changelog".into())
}
