//! Runtime primitives shared by specX hosts.
//!
//! Platform code resolves process-level locations and environment values. It
//! does not know about HTTP routes, database schemas, or domain permissions.

use std::{
    env,
    path::{Path, PathBuf},
};

/// Persistence target selected by a host from `SPECX_DATABASE_URL`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DatabaseUrl {
    /// A local SQLite database file.
    Sqlite(PathBuf),
    /// A PostgreSQL connection URL consumed by the persistence adapter.
    Postgres(String),
}

/// Parse the shared database URL syntax used by the Web and desktop hosts.
pub fn parse_database_url(raw: Option<&str>, data_dir: &Path) -> Result<DatabaseUrl, String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DatabaseUrl::Sqlite(data_dir.join("specx.sqlite")));
    };

    if raw.starts_with("postgres://") || raw.starts_with("postgresql://") {
        return Ok(DatabaseUrl::Postgres(raw.to_owned()));
    }

    if let Some(path) = raw.strip_prefix("sqlite://") {
        if path.is_empty() || path == ":memory:" || path.contains('?') || path.contains('#') {
            return Err("SPECX_DATABASE_URL must use sqlite://<path> or a PostgreSQL URL".to_owned());
        }
        return Ok(DatabaseUrl::Sqlite(PathBuf::from(path)));
    }

    Err("SPECX_DATABASE_URL must use sqlite://<path>, postgres://, or postgresql://".to_owned())
}

/// Filesystem locations used by a specX process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    /// Root for the local SQLite file and other application-owned state.
    pub data_dir: PathBuf,
    /// Directory used for local backup artifacts.
    pub backup_dir: PathBuf,
    /// Compiled browser assets served by the Web host.
    pub static_dir: PathBuf,
}

impl AppPaths {
    /// Read the conventional `SPECX_*` locations, keeping defaults suitable
    /// for both local development and the Docker image.
    pub fn from_env() -> Self {
        let data_dir = env::var_os("SPECX_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("data"));
        let backup_dir =
            env::var_os("SPECX_BACKUP_ROOT").map(PathBuf::from).unwrap_or_else(|| data_dir.join("backups"));
        let static_dir =
            env::var_os("SPECX_STATIC_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("apps/desktop/dist"));
        Self { data_dir, backup_dir, static_dir }
    }

    /// The default SQLite file for a host that did not select another store.
    pub fn sqlite_file(&self) -> PathBuf {
        self.data_dir.join("specx.sqlite")
    }
}

#[cfg(test)]
mod tests {
    use super::{AppPaths, DatabaseUrl, parse_database_url};
    use std::path::{Path, PathBuf};

    #[test]
    fn derives_the_default_sqlite_location() {
        let paths = AppPaths {
            data_dir: "/tmp/specx".into(),
            backup_dir: "/tmp/specx/backups".into(),
            static_dir: "static".into(),
        };
        assert_eq!(paths.sqlite_file(), std::path::PathBuf::from("/tmp/specx/specx.sqlite"));
    }

    #[test]
    fn selects_sqlite_by_default_and_postgres_by_scheme() {
        let data_dir = Path::new("/var/lib/specx");
        assert_eq!(parse_database_url(None, data_dir).unwrap(), DatabaseUrl::Sqlite(data_dir.join("specx.sqlite")));
        assert_eq!(
            parse_database_url(Some("sqlite:///tmp/specx.sqlite"), data_dir).unwrap(),
            DatabaseUrl::Sqlite(PathBuf::from("/tmp/specx.sqlite"))
        );
        assert_eq!(
            parse_database_url(Some("postgresql://user:pass@db/specx"), data_dir).unwrap(),
            DatabaseUrl::Postgres("postgresql://user:pass@db/specx".into())
        );
    }

    #[test]
    fn rejects_unsupported_or_ambiguous_database_urls() {
        assert!(parse_database_url(Some("mysql://db/specx"), Path::new("data")).is_err());
        assert!(parse_database_url(Some("sqlite://:memory:"), Path::new("data")).is_err());
    }
}
