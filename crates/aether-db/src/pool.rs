use std::path::Path;
use thiserror::Error;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct Pool {
    pub inner: SqlitePool,
}

impl Pool {
    /// Open (and create if missing) the per-tenant SQLite file at `path`.
    /// Enables WAL mode and applies all migrations from `crates/aether-db/migrations`.
    pub async fn open(path: &Path) -> Result<Self, PoolError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let options = SqliteConnectOptions::from_str(&url)?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(options)
            .await?;

        // Migrations live in `crates/aether-db/migrations` and are embedded
        // at compile time once they are added (PR #2).
        // sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Pool { inner: pool })
    }

    pub fn handle(&self) -> &SqlitePool {
        &self.inner
    }
}
