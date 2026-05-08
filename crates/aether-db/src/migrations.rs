//! SQLite migration registration.
//!
//! Migration files live in `./migrations/*.sql` (sqlx migration format).
//! At runtime they are applied by `Pool::open` once the migrate! macro
//! is enabled (PR #2). Until then this module documents the convention
//! and exposes a stub helper for tools that need to apply migrations
//! out-of-band.

/// Apply all pending migrations to the connection pool.
///
/// In a future PR this will call `sqlx::migrate!("./migrations").run(&pool)`.
pub async fn apply(_pool: &sqlx::SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    Ok(())
}
