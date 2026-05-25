//! `DbEvidenceSource` — the production [`EvidenceSource`] backed by the
//! per-tenant SQLite database (`aether-db`).
//!
//! Every probe in this crate consumes `EvidenceSource`; until now the
//! only implementor was the in-memory `MockEvidenceSource` used by tests.
//! This type runs the real queries against the compliance-evidence tables
//! from migration `0003_compliance_evidence.sql`, plus the existing
//! `sync_outbox` (for encrypted-write activity).
//!
//! ## Timestamps
//! All timestamp columns are RFC3339 UTC TEXT. Queries bind cutoffs with
//! [`DateTime::to_rfc3339`] so the comparison SQLite performs lexically
//! is also chronological — the same value the writer stored.
//!
//! ## "As of now" methods
//! `open_incidents_past_sla`, `breaches_unnotified_within`, and
//! `dsars_past_deadline` are defined relative to the present (an SLA is
//! "overdue as of now"), so they derive their cutoff from `Utc::now()`.
//! The point-in-time methods (`latest_*`, `*_since`) take their reference
//! from the caller and stay reproducible.

use crate::evidence::{EvidenceError, EvidenceSource, OpenIncident};
use aether_db::Pool;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

/// Production evidence source over a per-tenant SQLite pool.
pub struct DbEvidenceSource {
    pool: Pool,
}

impl DbEvidenceSource {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

fn q(e: sqlx::Error) -> EvidenceError {
    EvidenceError::Query(e.to_string())
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, EvidenceError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| EvidenceError::Query(format!("bad timestamp {s:?}: {e}")))
}

#[async_trait]
impl EvidenceSource for DbEvidenceSource {
    async fn audit_log_tampered_since(&self, since: DateTime<Utc>) -> Result<bool, EvidenceError> {
        let hit: i64 = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM audit_log_mutations WHERE attempted_at >= ?)",
        )
        .bind(since.to_rfc3339())
        .fetch_one(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(hit != 0)
    }

    async fn encrypted_column_writes_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<u64, EvidenceError> {
        // sync_outbox.created_at is an INTEGER unix-epoch column.
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_outbox WHERE encrypted = 1 AND created_at >= ?",
        )
        .bind(since.timestamp())
        .fetch_one(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(n as u64)
    }

    async fn open_incidents_past_sla(
        &self,
        sla_hours: u32,
    ) -> Result<Vec<OpenIncident>, EvidenceError> {
        let cutoff = Utc::now() - Duration::hours(i64::from(sla_hours));
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, opened_at, severity FROM security_incidents \
             WHERE resolved_at IS NULL AND opened_at <= ? ORDER BY opened_at",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_all(self.pool.handle())
        .await
        .map_err(q)?;

        rows.into_iter()
            .map(|(id, opened_at, severity)| {
                Ok(OpenIncident {
                    id,
                    opened_at: parse_ts(&opened_at)?,
                    severity,
                })
            })
            .collect()
    }

    async fn latest_key_rotation(&self) -> Result<Option<DateTime<Utc>>, EvidenceError> {
        let row: Option<String> = sqlx::query_scalar(
            "SELECT rotated_at FROM key_rotation_events ORDER BY rotated_at DESC LIMIT 1",
        )
        .fetch_optional(self.pool.handle())
        .await
        .map_err(q)?;
        row.map(|s| parse_ts(&s)).transpose()
    }

    async fn cold_chain_excursions_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<u64, EvidenceError> {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cold_chain_excursions WHERE occurred_at >= ?")
                .bind(since.to_rfc3339())
                .fetch_one(self.pool.handle())
                .await
                .map_err(q)?;
        Ok(n as u64)
    }

    async fn latest_review(
        &self,
        control_id: &str,
    ) -> Result<Option<DateTime<Utc>>, EvidenceError> {
        let row: Option<String> = sqlx::query_scalar(
            "SELECT reviewed_at FROM periodic_reviews WHERE control_id = ? \
             ORDER BY reviewed_at DESC LIMIT 1",
        )
        .bind(control_id)
        .fetch_optional(self.pool.handle())
        .await
        .map_err(q)?;
        row.map(|s| parse_ts(&s)).transpose()
    }

    async fn breaches_unnotified_within(&self, deadline_hours: u32) -> Result<u64, EvidenceError> {
        let cutoff = Utc::now() - Duration::hours(i64::from(deadline_hours));
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM data_breaches WHERE notified_at IS NULL AND detected_at <= ?",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_one(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(n as u64)
    }

    async fn dsars_past_deadline(&self, deadline_days: u32) -> Result<u64, EvidenceError> {
        let cutoff = Utc::now() - Duration::days(i64::from(deadline_days));
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM dsar_requests WHERE fulfilled_at IS NULL AND requested_at <= ?",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_one(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(n as u64)
    }

    async fn unbound_signatures(&self) -> Result<u64, EvidenceError> {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM electronic_signatures WHERE bound = 0")
                .fetch_one(self.pool.handle())
                .await
                .map_err(q)?;
        Ok(n as u64)
    }
}
