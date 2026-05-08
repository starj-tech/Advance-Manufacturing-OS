//! Local SQLite layer for AETHER-OS.
//!
//! One SQLite file per tenant under `<app_data>/aether/<tenant_id>/main.db`.
//! WAL mode is enabled so concurrent reads (UI + sync engine) do not block
//! writes from the protocol bridge ingest path.

pub mod migrations;
pub mod pool;

pub use pool::{Pool, PoolError};

/// SQLite filename inside a per-tenant directory.
pub const DB_FILENAME: &str = "main.db";
