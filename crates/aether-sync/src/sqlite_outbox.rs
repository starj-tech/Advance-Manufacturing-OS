use crate::outbox::{Op, Outbox, OutboxEntry, OutboxError};
use aether_core::Hlc;
use aether_db::Pool;
use sqlx::Row;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLite-backed `Outbox` implementation.
///
/// Backed by the `sync_outbox` table in `crates/aether-db/migrations/0001_init.sql`:
///
/// ```text
/// id          INTEGER PRIMARY KEY AUTOINCREMENT  -- FIFO ordering
/// op_id       TEXT NOT NULL UNIQUE               -- idempotent enqueue
/// entity      TEXT NOT NULL
/// entity_id   TEXT NOT NULL
/// op          TEXT NOT NULL                      -- 'insert' | 'update' | 'delete' | 'crdt_patch'
/// payload     BLOB                               -- ciphertext or raw JSON
/// hlc_ts      TEXT NOT NULL                      -- HLC wire format
/// parent_hlc  TEXT                               -- optional parent HLC for OCC
/// encrypted   INTEGER NOT NULL DEFAULT 0
/// attempts    INTEGER NOT NULL DEFAULT 0         -- bumped by mark_failed
/// last_error  TEXT
/// created_at  INTEGER NOT NULL                   -- unix millis
/// ```
///
/// Lifecycle:
///   - `enqueue` inserts a row; duplicate `op_id` is idempotent.
///   - `poll(limit)` returns the oldest `limit` rows ordered by `id ASC`.
///   - `mark_done(op_ids)` deletes the rows; the queue shrinks.
///   - `mark_failed(op_id, error)` bumps `attempts` and stores `last_error`
///     so the row stays in the queue for next poll.
pub struct SqliteOutbox {
    pool: Pool,
}

impl SqliteOutbox {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

fn op_to_str(op: &Op) -> &'static str {
    match op {
        Op::Insert => "insert",
        Op::Update => "update",
        Op::Delete => "delete",
        Op::CrdtPatch => "crdt_patch",
    }
}

fn op_from_str(s: &str) -> Result<Op, OutboxError> {
    match s {
        "insert" => Ok(Op::Insert),
        "update" => Ok(Op::Update),
        "delete" => Ok(Op::Delete),
        "crdt_patch" => Ok(Op::CrdtPatch),
        other => Err(OutboxError::Encoding(format!("unknown op `{}`", other))),
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[async_trait::async_trait]
impl Outbox for SqliteOutbox {
    async fn enqueue(&self, entry: OutboxEntry) -> Result<(), OutboxError> {
        let parent = entry.parent_hlc.as_ref().map(|h| h.to_string());
        // ON CONFLICT(op_id) DO NOTHING — duplicate enqueues are
        // idempotent (typical case: client retries after a crash before
        // an Ack landed).
        let res = sqlx::query(
            "INSERT INTO sync_outbox \
             (op_id, entity, entity_id, op, payload, hlc_ts, parent_hlc, encrypted, attempts, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9) \
             ON CONFLICT(op_id) DO NOTHING",
        )
        .bind(&entry.op_id)
        .bind(&entry.entity)
        .bind(&entry.entity_id)
        .bind(op_to_str(&entry.op))
        .bind(&entry.payload)
        .bind(entry.hlc_ts.to_string())
        .bind(parent)
        .bind(if entry.encrypted { 1_i64 } else { 0_i64 })
        .bind(now_millis())
        .execute(self.pool.handle())
        .await
        .map_err(|e| OutboxError::Storage(e.to_string()))?;

        tracing::trace!(
            op_id = %entry.op_id,
            entity = %entry.entity,
            inserted = res.rows_affected() == 1,
            "outbox enqueue"
        );
        Ok(())
    }

    async fn poll(&self, limit: u32) -> Result<Vec<OutboxEntry>, OutboxError> {
        let rows = sqlx::query(
            "SELECT op_id, entity, entity_id, op, payload, hlc_ts, parent_hlc, encrypted \
             FROM sync_outbox \
             ORDER BY id ASC \
             LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(self.pool.handle())
        .await
        .map_err(|e| OutboxError::Storage(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let op_id: String = r.try_get("op_id").map_err(enc)?;
            let entity: String = r.try_get("entity").map_err(enc)?;
            let entity_id: String = r.try_get("entity_id").map_err(enc)?;
            let op_str: String = r.try_get("op").map_err(enc)?;
            let payload: Vec<u8> = r
                .try_get::<Option<Vec<u8>>, _>("payload")
                .map_err(enc)?
                .unwrap_or_default();
            let hlc_str: String = r.try_get("hlc_ts").map_err(enc)?;
            let parent_str: Option<String> = r.try_get("parent_hlc").map_err(enc)?;
            let encrypted: i64 = r.try_get("encrypted").map_err(enc)?;

            let hlc_ts = Hlc::parse(&hlc_str)
                .ok_or_else(|| OutboxError::Encoding(format!("malformed hlc_ts `{}`", hlc_str)))?;
            let parent_hlc = match parent_str {
                Some(s) => Some(Hlc::parse(&s).ok_or_else(|| {
                    OutboxError::Encoding(format!("malformed parent_hlc `{}`", s))
                })?),
                None => None,
            };

            out.push(OutboxEntry {
                op_id,
                entity,
                entity_id,
                op: op_from_str(&op_str)?,
                payload,
                hlc_ts,
                parent_hlc,
                encrypted: encrypted != 0,
            });
        }
        Ok(out)
    }

    async fn mark_done(&self, op_ids: &[String]) -> Result<(), OutboxError> {
        if op_ids.is_empty() {
            return Ok(());
        }
        // SQLite has no array binding; build a single transaction that
        // executes one DELETE per op_id. Cheaper than rebuilding a
        // dynamic IN-clause and avoids quoting pitfalls.
        let mut tx = self
            .pool
            .handle()
            .begin()
            .await
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        for id in op_ids {
            sqlx::query("DELETE FROM sync_outbox WHERE op_id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| OutboxError::Storage(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn mark_failed(&self, op_id: &str, error: &str) -> Result<(), OutboxError> {
        sqlx::query(
            "UPDATE sync_outbox \
             SET attempts = attempts + 1, last_error = ?2 \
             WHERE op_id = ?1",
        )
        .bind(op_id)
        .bind(error)
        .execute(self.pool.handle())
        .await
        .map_err(|e| OutboxError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn size(&self) -> Result<u32, OutboxError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sync_outbox")
            .fetch_one(self.pool.handle())
            .await
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        Ok(row.0 as u32)
    }
}

fn enc<E: std::fmt::Display>(e: E) -> OutboxError {
    OutboxError::Encoding(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(op_id: &str, entity_id: &str, hlc: Hlc) -> OutboxEntry {
        OutboxEntry {
            op_id: op_id.into(),
            entity: "work_orders".into(),
            entity_id: entity_id.into(),
            op: Op::Insert,
            payload: b"{\"qty\":42}".to_vec(),
            hlc_ts: hlc,
            parent_hlc: None,
            encrypted: false,
        }
    }

    async fn fresh() -> SqliteOutbox {
        let pool = Pool::open_in_memory().await.expect("in-memory pool");
        SqliteOutbox::new(pool)
    }

    #[tokio::test]
    async fn enqueue_then_size_returns_one() {
        let ob = fresh().await;
        let h = Hlc::new(1, 0, "node-a");
        ob.enqueue(entry("op-1", "wo-1", h)).await.unwrap();
        assert_eq!(ob.size().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn duplicate_op_id_is_idempotent() {
        let ob = fresh().await;
        let h = Hlc::new(1, 0, "node-a");
        ob.enqueue(entry("op-1", "wo-1", h.clone())).await.unwrap();
        ob.enqueue(entry("op-1", "wo-1", h)).await.unwrap();
        assert_eq!(
            ob.size().await.unwrap(),
            1,
            "duplicate op_id must not double-insert"
        );
    }

    #[tokio::test]
    async fn poll_returns_fifo_order() {
        let ob = fresh().await;
        for i in 0..5 {
            let h = Hlc::new(i, 0, "node-a");
            ob.enqueue(entry(&format!("op-{}", i), &format!("wo-{}", i), h))
                .await
                .unwrap();
        }
        let polled = ob.poll(10).await.unwrap();
        let order: Vec<_> = polled.iter().map(|e| e.op_id.as_str()).collect();
        assert_eq!(order, vec!["op-0", "op-1", "op-2", "op-3", "op-4"]);
    }

    #[tokio::test]
    async fn poll_respects_limit() {
        let ob = fresh().await;
        for i in 0..5 {
            let h = Hlc::new(i, 0, "node-a");
            ob.enqueue(entry(&format!("op-{}", i), &format!("wo-{}", i), h))
                .await
                .unwrap();
        }
        let polled = ob.poll(2).await.unwrap();
        assert_eq!(polled.len(), 2);
        assert_eq!(polled[0].op_id, "op-0");
        assert_eq!(polled[1].op_id, "op-1");
    }

    #[tokio::test]
    async fn mark_done_removes_entries_and_shrinks_queue() {
        let ob = fresh().await;
        for i in 0..3 {
            let h = Hlc::new(i, 0, "node-a");
            ob.enqueue(entry(&format!("op-{}", i), &format!("wo-{}", i), h))
                .await
                .unwrap();
        }
        assert_eq!(ob.size().await.unwrap(), 3);

        ob.mark_done(&["op-0".into(), "op-2".into()]).await.unwrap();
        assert_eq!(ob.size().await.unwrap(), 1);

        let remaining = ob.poll(10).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].op_id, "op-1");
    }

    #[tokio::test]
    async fn mark_failed_keeps_entry_and_bumps_attempts() {
        let ob = fresh().await;
        let h = Hlc::new(1, 0, "node-a");
        ob.enqueue(entry("op-1", "wo-1", h)).await.unwrap();

        ob.mark_failed("op-1", "network: timeout").await.unwrap();
        ob.mark_failed("op-1", "network: timeout").await.unwrap();

        // Still in queue.
        assert_eq!(ob.size().await.unwrap(), 1);

        // Confirm attempts via raw query — the trait surface doesn't
        // expose attempts but the schema does, so this guards the
        // contract that mark_failed actually increments.
        let attempts: (i64,) = sqlx::query_as("SELECT attempts FROM sync_outbox WHERE op_id = ?1")
            .bind("op-1")
            .fetch_one(ob.pool.handle())
            .await
            .unwrap();
        assert_eq!(attempts.0, 2);
    }

    #[tokio::test]
    async fn mark_done_with_empty_slice_is_noop() {
        let ob = fresh().await;
        let h = Hlc::new(1, 0, "node-a");
        ob.enqueue(entry("op-1", "wo-1", h)).await.unwrap();

        ob.mark_done(&[]).await.unwrap();
        assert_eq!(ob.size().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn poll_round_trips_hlc_payload_and_parent() {
        let ob = fresh().await;
        let parent = Hlc::new(10, 0, "node-a");
        let now = Hlc::new(11, 5, "node-b");
        let entry = OutboxEntry {
            op_id: "op-1".into(),
            entity: "boms".into(),
            entity_id: "bom-1".into(),
            op: Op::CrdtPatch,
            payload: vec![0xde, 0xad, 0xbe, 0xef],
            hlc_ts: now.clone(),
            parent_hlc: Some(parent.clone()),
            encrypted: true,
        };
        ob.enqueue(entry).await.unwrap();

        let polled = ob.poll(1).await.unwrap();
        assert_eq!(polled.len(), 1);
        let p = &polled[0];
        assert_eq!(p.op_id, "op-1");
        assert_eq!(p.entity, "boms");
        assert_eq!(p.op, Op::CrdtPatch);
        assert_eq!(p.payload, vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(p.hlc_ts, now);
        assert_eq!(p.parent_hlc, Some(parent));
        assert!(p.encrypted);
    }
}
