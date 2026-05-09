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

/// Migrations bundled at compile time from `crates/aether-db/migrations/`.
/// Forward-only; sqlx tracks applied migrations in `_sqlx_migrations`.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Pool {
    pub inner: SqlitePool,
}

impl Pool {
    /// Open (and create if missing) the per-tenant SQLite file at `path`.
    /// Enables WAL mode and applies all bundled migrations.
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

        MIGRATOR.run(&pool).await?;
        tracing::info!(?path, "aether-db pool ready, migrations applied");

        Ok(Pool { inner: pool })
    }

    /// In-memory SQLite for unit/integration tests. WAL mode is not
    /// supported on `:memory:` — tests use the default rollback journal.
    /// Single-connection pool because every fresh connection to
    /// `sqlite::memory:` opens a brand-new database.
    pub async fn open_in_memory() -> Result<Self, PoolError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        MIGRATOR.run(&pool).await?;
        Ok(Pool { inner: pool })
    }

    pub fn handle(&self) -> &SqlitePool {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_pool_runs_all_migrations() {
        let pool = Pool::open_in_memory()
            .await
            .expect("in-memory pool should open");

        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool.inner)
            .await
            .expect("_sqlx_migrations table should exist after migrate");
        assert!(row.0 >= 1, "expected at least one migration applied");

        // Spot-check a real domain table from 0001_init.sql.
        let work_orders: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM work_orders")
            .fetch_one(&pool.inner)
            .await
            .expect("work_orders table should exist");
        assert_eq!(work_orders.0, 0);
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let pool = Pool::open_in_memory().await.unwrap();
        // Running the migrator a second time against the same pool must
        // be a no-op (sqlx checks `_sqlx_migrations` first).
        MIGRATOR
            .run(&pool.inner)
            .await
            .expect("running migrations twice should not error");
    }
}
