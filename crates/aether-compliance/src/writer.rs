//! `EvidenceWriter` — the operational write path into the compliance-
//! evidence tables (migration `0003_compliance_evidence.sql`).
//!
//! `DbEvidenceSource` reads these tables; something has to fill them. The
//! operational layers call these helpers when a compliance-relevant event
//! happens: a key is rotated, a breach is detected/notified, a DSAR is
//! filed/fulfilled, a cold-chain reading leaves the band, an audit-log
//! mutation is attempted, a review is completed, an e-signature is
//! recorded. Keeping the inserts here (rather than scattered raw SQL)
//! gives one typed, tested surface and pins the RFC3339 timestamp format
//! that the read side compares against.
//!
//! These are plain inserts/updates — no business logic. The probe layer
//! owns interpretation; this layer only records facts.

use crate::evidence::EvidenceError;
use aether_db::Pool;
use chrono::{DateTime, Utc};

fn q(e: sqlx::Error) -> EvidenceError {
    EvidenceError::Query(e.to_string())
}

/// Writes compliance evidence rows into the per-tenant SQLite database.
pub struct EvidenceWriter {
    pool: Pool,
}

impl EvidenceWriter {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Record an attempted UPDATE/DELETE against the append-only audit
    /// log. `op` must be `"update"` or `"delete"`.
    pub async fn record_audit_mutation(
        &self,
        op: &str,
        detail: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO audit_log_mutations (attempted_at, op, detail) VALUES (?, ?, ?)")
            .bind(at.to_rfc3339())
            .bind(op)
            .bind(detail)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Open a security/safety incident.
    pub async fn open_incident(
        &self,
        id: &str,
        severity: &str,
        opened_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO security_incidents (id, opened_at, severity) VALUES (?, ?, ?)")
            .bind(id)
            .bind(opened_at.to_rfc3339())
            .bind(severity)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Mark a previously-opened incident resolved.
    pub async fn resolve_incident(
        &self,
        id: &str,
        resolved_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("UPDATE security_incidents SET resolved_at = ? WHERE id = ?")
            .bind(resolved_at.to_rfc3339())
            .bind(id)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Record a data-encryption-key rotation.
    pub async fn record_key_rotation(&self, at: DateTime<Utc>) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO key_rotation_events (rotated_at) VALUES (?)")
            .bind(at.to_rfc3339())
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Record a completed periodic review for a control point.
    pub async fn record_review(
        &self,
        control_id: &str,
        reviewed_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO periodic_reviews (control_id, reviewed_at) VALUES (?, ?)")
            .bind(control_id)
            .bind(reviewed_at.to_rfc3339())
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Record a cold-chain temperature excursion.
    pub async fn record_cold_chain_excursion(
        &self,
        occurred_at: DateTime<Utc>,
        machine_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), EvidenceError> {
        sqlx::query(
            "INSERT INTO cold_chain_excursions (occurred_at, machine_id, detail) VALUES (?, ?, ?)",
        )
        .bind(occurred_at.to_rfc3339())
        .bind(machine_id)
        .bind(detail)
        .execute(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(())
    }

    /// Record a detected personal-data breach (not yet notified).
    pub async fn record_breach(
        &self,
        id: &str,
        detected_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO data_breaches (id, detected_at) VALUES (?, ?)")
            .bind(id)
            .bind(detected_at.to_rfc3339())
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Mark a breach as notified to the supervisory authority.
    pub async fn mark_breach_notified(
        &self,
        id: &str,
        notified_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("UPDATE data_breaches SET notified_at = ? WHERE id = ?")
            .bind(notified_at.to_rfc3339())
            .bind(id)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Record a data-subject access request (not yet fulfilled).
    pub async fn record_dsar(
        &self,
        id: &str,
        requested_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("INSERT INTO dsar_requests (id, requested_at) VALUES (?, ?)")
            .bind(id)
            .bind(requested_at.to_rfc3339())
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Mark a DSAR fulfilled.
    pub async fn mark_dsar_fulfilled(
        &self,
        id: &str,
        fulfilled_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("UPDATE dsar_requests SET fulfilled_at = ? WHERE id = ?")
            .bind(fulfilled_at.to_rfc3339())
            .bind(id)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Open a violation against a control point (a non-conformance, an
    /// open CAPA, an uncorrected out-of-spec event, …).
    pub async fn open_violation(
        &self,
        id: &str,
        control_id: &str,
        opened_at: DateTime<Utc>,
        detail: Option<&str>,
    ) -> Result<(), EvidenceError> {
        sqlx::query(
            "INSERT INTO control_violations (id, control_id, opened_at, detail) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(control_id)
        .bind(opened_at.to_rfc3339())
        .bind(detail)
        .execute(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(())
    }

    /// Resolve a previously-opened control violation.
    pub async fn resolve_violation(
        &self,
        id: &str,
        resolved_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query("UPDATE control_violations SET resolved_at = ? WHERE id = ?")
            .bind(resolved_at.to_rfc3339())
            .bind(id)
            .execute(self.pool.handle())
            .await
            .map_err(q)?;
        Ok(())
    }

    /// Record an electronic signature. `bound` is whether it is
    /// cryptographically tied to its record.
    pub async fn record_signature(
        &self,
        id: &str,
        record_ref: &str,
        bound: bool,
        signed_at: DateTime<Utc>,
    ) -> Result<(), EvidenceError> {
        sqlx::query(
            "INSERT INTO electronic_signatures (id, record_ref, bound, signed_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(record_ref)
        .bind(i64::from(bound))
        .bind(signed_at.to_rfc3339())
        .execute(self.pool.handle())
        .await
        .map_err(q)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_evidence::DbEvidenceSource;
    use crate::evidence::EvidenceSource;
    use chrono::Duration;

    async fn pool() -> Pool {
        Pool::open_in_memory().await.expect("in-memory pool")
    }

    #[tokio::test]
    async fn audit_mutation_round_trips() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);
        let now = Utc::now();

        assert!(!src
            .audit_log_tampered_since(now - Duration::hours(1))
            .await
            .unwrap());
        w.record_audit_mutation("delete", Some("manual edit"), now)
            .await
            .unwrap();
        assert!(src
            .audit_log_tampered_since(now - Duration::hours(1))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn incident_open_then_resolve() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);
        let opened = Utc::now() - Duration::hours(100);

        w.open_incident("inc-1", "high", opened).await.unwrap();
        assert_eq!(src.open_incidents_past_sla(72).await.unwrap().len(), 1);

        w.resolve_incident("inc-1", Utc::now()).await.unwrap();
        assert!(src.open_incidents_past_sla(72).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn key_rotation_round_trips() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);
        let at = Utc::now() - Duration::days(5);

        assert_eq!(src.latest_key_rotation().await.unwrap(), None);
        w.record_key_rotation(at).await.unwrap();
        assert!(src.latest_key_rotation().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn review_round_trips() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);

        w.record_review("is-access-review", Utc::now() - Duration::days(10))
            .await
            .unwrap();
        assert!(src
            .latest_review("is-access-review")
            .await
            .unwrap()
            .is_some());
        assert_eq!(src.latest_review("qms-internal-audit").await.unwrap(), None);
    }

    #[tokio::test]
    async fn cold_chain_round_trips() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);
        let now = Utc::now();

        w.record_cold_chain_excursion(now, Some("FRIDGE-1"), None)
            .await
            .unwrap();
        assert_eq!(
            src.cold_chain_excursions_since(now - Duration::hours(1))
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn breach_detect_then_notify() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);

        w.record_breach("br-1", Utc::now() - Duration::hours(100))
            .await
            .unwrap();
        assert_eq!(src.breaches_unnotified_within(72).await.unwrap(), 1);

        w.mark_breach_notified("br-1", Utc::now()).await.unwrap();
        assert_eq!(src.breaches_unnotified_within(72).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn dsar_request_then_fulfil() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);

        w.record_dsar("dsar-1", Utc::now() - Duration::days(40))
            .await
            .unwrap();
        assert_eq!(src.dsars_past_deadline(30).await.unwrap(), 1);

        w.mark_dsar_fulfilled("dsar-1", Utc::now()).await.unwrap();
        assert_eq!(src.dsars_past_deadline(30).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn signature_bound_and_unbound() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);

        w.record_signature("sig-1", "rec-1", true, Utc::now())
            .await
            .unwrap();
        assert_eq!(src.unbound_signatures().await.unwrap(), 0);

        w.record_signature("sig-2", "rec-2", false, Utc::now())
            .await
            .unwrap();
        assert_eq!(src.unbound_signatures().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn violation_open_then_resolve() {
        let p = pool().await;
        let w = EvidenceWriter::new(p.clone());
        let src = DbEvidenceSource::new(p);

        w.open_violation(
            "nc-1",
            "qms-non-conformance",
            Utc::now(),
            Some("scrap rate"),
        )
        .await
        .unwrap();
        assert_eq!(src.open_violations("qms-non-conformance").await.unwrap(), 1);
        // A different control is unaffected.
        assert_eq!(src.open_violations("qms-capa").await.unwrap(), 0);

        w.resolve_violation("nc-1", Utc::now()).await.unwrap();
        assert_eq!(src.open_violations("qms-non-conformance").await.unwrap(), 0);
    }
}
