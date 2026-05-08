use aether_core::Hlc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Insert,
    Update,
    Delete,
    CrdtPatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub op_id: String,
    pub entity: String,
    pub entity_id: String,
    pub op: Op,
    /// Opaque payload — ciphertext for encrypted entities, JSON otherwise.
    pub payload: Vec<u8>,
    pub hlc_ts: Hlc,
    pub parent_hlc: Option<Hlc>,
    pub encrypted: bool,
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("storage: {0}")]
    Storage(String),
    #[error("encoding: {0}")]
    Encoding(String),
}

/// Trait abstracting the local outbox queue. Backed by SQLite in
/// production (`aether_db::Pool`); a fake impl is used in tests.
#[async_trait::async_trait]
pub trait Outbox: Send + Sync {
    async fn enqueue(&self, entry: OutboxEntry) -> Result<(), OutboxError>;
    async fn poll(&self, limit: u32) -> Result<Vec<OutboxEntry>, OutboxError>;
    async fn mark_done(&self, op_ids: &[String]) -> Result<(), OutboxError>;
    async fn mark_failed(&self, op_id: &str, error: &str) -> Result<(), OutboxError>;
    async fn size(&self) -> Result<u32, OutboxError>;
}
