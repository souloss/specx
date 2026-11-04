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
    let mut api = match database {
        config::DatabaseUrl::Sqlite(path) => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            ApiState::open(path)?
        }
        config::DatabaseUrl::Postgres(url) => {
            #[cfg(feature = "postgres")]
            {
                let pool_size =
                    env::var("SPECX_DATABASE_MAX_POOL_SIZE").ok().and_then(|value| value.parse().ok()).unwrap_or(16);
                ApiState::open_postgres_with_backup_root(&url, pool_size, &paths.backup_dir).await?
            }
            #[cfg(not(feature = "postgres"))]
            {
                let _ = url;
                return Err(
                    "SPECX_DATABASE_URL selects PostgreSQL, but this binary was built without the postgres feature"
                        .into(),
                );
            }
        }
    };
    if let Some(origin) = env::var("SPECX_PUBLIC_ORIGIN").ok().filter(|value| !value.trim().is_empty()) {
        api = api.with_public_origin(&origin)?;
    }
    api = api.with_backup_storage_from_env().await?;

    let password = env::var("SPECX_PASSWORD").map_err(|_| "SPECX_PASSWORD must be set")?;
    if password.len() < 12 {
        return Err("SPECX_PASSWORD must contain at least 12 characters".into());
    }
    let host = auth::HostState::new(
        password,
        env::var("SPECX_SESSION_SECRET").unwrap_or_else(|_| "change-this-session-secret".into()),
    );

    let static_index = paths.static_dir.join("index.html");
    let api_router = specx_api::router::<auth::HostState>(api);
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
