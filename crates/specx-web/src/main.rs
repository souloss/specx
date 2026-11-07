mod auth;
mod config;

use std::{env, net::SocketAddr};

use axum::{Router, middleware};
use specx_api::ApiState;
use specx_platform::AppPaths;
use tokio::net::TcpListener;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "specx_web=info,tower_http=info".into()))
        .init();

    let paths = AppPaths::from_env();
    tokio::fs::create_dir_all(&paths.data_dir).await?;
    tokio::fs::create_dir_all(&paths.backup_dir).await?;

    let database = config::parse_database_url(env::var("SPECX_DATABASE_URL").ok().as_deref(), &paths.data_dir)
        .map_err(|error| format!("invalid database configuration: {error}"))?;
    let mut api = open_api_with_retry(&database, &paths).await?;
    if let Some(origin) = env::var("SPECX_PUBLIC_ORIGIN").ok().filter(|value| !value.trim().is_empty()) {
        api = api.with_public_origin(&origin)?;
    }
    api = api.with_backup_storage_from_env().await?;
    api = api.with_webhook_secret(env::var("SPECX_WEBHOOK_SECRET").ok());

    let password = env::var("SPECX_PASSWORD").map_err(|_| "SPECX_PASSWORD must be set")?;
    if password.len() < 12 {
        return Err("SPECX_PASSWORD must contain at least 12 characters".into());
    }
    let host = auth::HostState::new(
        password,
        env::var("SPECX_SESSION_SECRET").unwrap_or_else(|_| "change-this-session-secret".into()),
    );

    let static_index = paths.static_dir.join("index.html");
    let worker_api = api.clone();
    let api_router = specx_api::router::<auth::HostState>(api);
    let worker_interval = env::var("SPECX_CONTROLLER_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5)
        .max(1);
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(worker_interval);
        loop {
            if let Err(error) = worker_api.run_repository_poller_once(100).await {
                tracing::warn!(%error, "repository poller iteration failed");
            }
            if let Err(error) = worker_api.run_controller_once(16).await {
                tracing::warn!(%error, "controller worker iteration failed");
            }
            tokio::time::sleep(interval).await;
        }
    });
    let app = Router::new()
        .merge(auth::router())
        .nest("/api/specx", api_router)
        .fallback_service(ServeDir::new(&paths.static_dir).fallback(ServeFile::new(static_index)))
        .layer(middleware::from_fn_with_state(host.clone(), auth::request_context))
        .layer(TraceLayer::new_for_http())
        .with_state(host);

    let bind = env::var("SPECX_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = env::var("SPECX_PORT").ok().and_then(|value| value.parse().ok()).unwrap_or(4224);
    let listener = TcpListener::bind(SocketAddr::new(bind.parse()?, port)).await?;
    tracing::info!("specX web listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

const MIGRATION_RETRY_ATTEMPTS: u32 = 3;
const MIGRATION_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

async fn open_api_with_retry(
    database: &config::DatabaseUrl,
    paths: &AppPaths,
) -> Result<ApiState, Box<dyn std::error::Error>> {
    #[cfg(not(feature = "postgres"))]
    if matches!(database, config::DatabaseUrl::Postgres(_)) {
        return Err(
            "SPECX_DATABASE_URL selects PostgreSQL, but this binary was built without the postgres feature".into()
        );
    }

    let mut last_error = None;
    for attempt in 1..=MIGRATION_RETRY_ATTEMPTS {
        let result = match database {
            config::DatabaseUrl::Sqlite(path) => {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                ApiState::open(path).map_err(|error| error.to_string())
            }
            config::DatabaseUrl::Postgres(url) => {
                #[cfg(feature = "postgres")]
                {
                    let pool_size = env::var("SPECX_DATABASE_MAX_POOL_SIZE")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(16);
                    ApiState::open_postgres_with_backup_root(url, pool_size, &paths.backup_dir)
                        .await
                        .map_err(|error| error.to_string())
                }
                #[cfg(not(feature = "postgres"))]
                {
                    let _ = url;
                    Err("SPECX_DATABASE_URL selects PostgreSQL, but this binary was built without the postgres feature"
                        .to_owned())
                }
            }
        };
        match result {
            Ok(api) => {
                tracing::info!(attempt, "storage migrations are ready");
                return Ok(api);
            }
            Err(error) => {
                tracing::warn!(attempt, max_attempts = MIGRATION_RETRY_ATTEMPTS, %error, "storage migration attempt failed");
                last_error = Some(error);
                if attempt < MIGRATION_RETRY_ATTEMPTS {
                    tokio::time::sleep(MIGRATION_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(format!(
        "storage migrations failed after {MIGRATION_RETRY_ATTEMPTS} attempts; refusing to bind business handlers: {}",
        last_error.unwrap_or_else(|| "unknown storage error".to_owned())
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::{MIGRATION_RETRY_ATTEMPTS, open_api_with_retry};
    use crate::config::DatabaseUrl;
    use specx_platform::AppPaths;

    #[tokio::test]
    async fn failed_storage_open_retries_before_refusing_startup() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("invalid.sqlite");
        std::fs::write(&database, b"not a SQLite database").unwrap();
        let paths = AppPaths {
            data_dir: directory.path().to_path_buf(),
            backup_dir: directory.path().join("backups"),
            static_dir: directory.path().join("static"),
        };

        let error = match open_api_with_retry(&DatabaseUrl::Sqlite(database), &paths).await {
            Ok(_) => panic!("invalid database unexpectedly opened"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains(&format!("failed after {MIGRATION_RETRY_ATTEMPTS} attempts")));
        assert!(error.contains("refusing to bind business handlers"));
    }
}
