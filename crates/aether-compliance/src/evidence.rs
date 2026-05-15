//! Evidence sources — read-only views the compliance probes consume.
//!
//! ## Why an abstraction layer
//! Concrete probes need to ask questions like "has the audit log been
//! tampered with since the last report?" or "are there encrypted-column
//! writes in the last 24h?". Hard-coding those questions against
//! `aether-db` would tie the compliance crate to SQLite/Postgres
//! specifics and make probes nearly impossible to test in isolation.
//! Instead, every "look at the running system" call goes through the
//! `EvidenceSource` trait. Production wraps `aether-db::Pool`; tests
//! ship the `MockEvidenceSource` below with canned answers.
//!
//! ## What lives on this trait vs. inside a probe
//! The trait surface is the *minimal* set of evidence primitives a
//! probe can compose. Anything probe-specific — interpreting whether
//! "12 incidents past SLA" is acceptable for a given standard, for
//! example — lives in the probe itself. The trait answers neutral
//! questions; the probe applies the policy.
//!
//! ## Determinism
//! Every method is parameterised by a `since: DateTime<Utc>` cutoff so
//! reports produced for the same window return the same evidence
//! regardless of when the probe runs. Probes are required to be
//! reproducible (see `control.rs` trait docs); the cutoff is how we
//! preserve that property even as the live database grows.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvidenceError {
    #[error("data source unavailable: {0}")]
    Unavailable(String),
    #[error("query: {0}")]
    Query(String),
}

/// Compact summary of an open incident — just enough for a probe to
/// decide pass/fail without leaking PII. Probes that need richer
/// context build queries on top of this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenIncident {
    pub id: String,
    pub opened_at: DateTime<Utc>,
    /// Severity bucket — `low` / `medium` / `high` / `critical`.
    pub severity: String,
}

#[async_trait]
pub trait EvidenceSource: Send + Sync {
    /// Has the immutable audit log seen any UPDATE or DELETE statement
    /// since `since`? Production checks `audit_log_mutations` view
    /// (an `INSERT`-only table that records any attempted mutation to
    /// the underlying `audit_log`). Returns `true` if tampering was
    /// detected.
    async fn audit_log_tampered_since(&self, since: DateTime<Utc>) -> Result<bool, EvidenceError>;

    /// How many encrypted-column writes (any column with the
    /// `encrypted=true` outbox marker) have landed since `since`?
    /// Used by encryption-at-rest probes to confirm the encryption
    /// pipeline is live, not just configured.
    async fn encrypted_column_writes_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<u64, EvidenceError>;

    /// Open incidents whose age exceeds `sla_hours`. Empty vec means
    /// the SLA is being honored. Production reads from a security-
    /// incident table; the per-incident triage detail lives elsewhere.
    async fn open_incidents_past_sla(
        &self,
        sla_hours: u32,
    ) -> Result<Vec<OpenIncident>, EvidenceError>;
}

/// In-memory `EvidenceSource` for tests and dry-run preview UI. Each
/// answer is set ahead of time; calls record the inputs so a test can
/// assert "the probe asked the right question with the right cutoff".
pub struct MockEvidenceSource {
    tampered: Mutex<bool>,
    encrypted_writes: Mutex<u64>,
    incidents: Mutex<Vec<OpenIncident>>,
    /// Map of method name → most recent `since` argument the probe
    /// passed. Exposed for tests; production never reads it.
    calls: Mutex<BTreeMap<&'static str, DateTime<Utc>>>,
    /// If `Some`, the next call to ANY method returns this error
    /// instead of the canned answer. Used to test dependency-missing
    /// handling without writing a second mock.
    fail_with: Mutex<Option<String>>,
}

impl MockEvidenceSource {
    pub fn new() -> Self {
        Self {
            tampered: Mutex::new(false),
            encrypted_writes: Mutex::new(0),
            incidents: Mutex::new(Vec::new()),
            calls: Mutex::new(BTreeMap::new()),
            fail_with: Mutex::new(None),
        }
    }

    pub fn set_tampered(&self, v: bool) {
        *self.tampered.lock().unwrap() = v;
    }

    pub fn set_encrypted_writes(&self, n: u64) {
        *self.encrypted_writes.lock().unwrap() = n;
    }

    pub fn set_open_incidents(&self, items: Vec<OpenIncident>) {
        *self.incidents.lock().unwrap() = items;
    }

    pub fn fail_next(&self, msg: impl Into<String>) {
        *self.fail_with.lock().unwrap() = Some(msg.into());
    }

    pub fn last_since(&self, method: &'static str) -> Option<DateTime<Utc>> {
        self.calls.lock().unwrap().get(method).copied()
    }

    /// Try to consume a queued failure. Internal helper that every
    /// trait method calls first.
    fn try_fail(&self) -> Result<(), EvidenceError> {
        if let Some(msg) = self.fail_with.lock().unwrap().take() {
            return Err(EvidenceError::Unavailable(msg));
        }
        Ok(())
    }
}

impl Default for MockEvidenceSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EvidenceSource for MockEvidenceSource {
    async fn audit_log_tampered_since(&self, since: DateTime<Utc>) -> Result<bool, EvidenceError> {
        self.calls
            .lock()
            .unwrap()
            .insert("audit_log_tampered_since", since);
        self.try_fail()?;
        Ok(*self.tampered.lock().unwrap())
    }

    async fn encrypted_column_writes_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<u64, EvidenceError> {
        self.calls
            .lock()
            .unwrap()
            .insert("encrypted_column_writes_since", since);
        self.try_fail()?;
        Ok(*self.encrypted_writes.lock().unwrap())
    }

    async fn open_incidents_past_sla(
        &self,
        _sla_hours: u32,
    ) -> Result<Vec<OpenIncident>, EvidenceError> {
        self.calls
            .lock()
            .unwrap()
            .insert("open_incidents_past_sla", Utc::now());
        self.try_fail()?;
        Ok(self.incidents.lock().unwrap().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_returns_canned_answers() {
        let m = MockEvidenceSource::new();
        m.set_tampered(true);
        m.set_encrypted_writes(42);
        m.set_open_incidents(vec![OpenIncident {
            id: "INC-1".into(),
            opened_at: Utc::now(),
            severity: "high".into(),
        }]);

        let cutoff = Utc::now();
        assert!(m.audit_log_tampered_since(cutoff).await.unwrap());
        assert_eq!(m.encrypted_column_writes_since(cutoff).await.unwrap(), 42);
        assert_eq!(m.open_incidents_past_sla(24).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mock_records_since_argument() {
        let m = MockEvidenceSource::new();
        let cutoff = Utc::now();
        let _ = m.audit_log_tampered_since(cutoff).await;
        assert_eq!(m.last_since("audit_log_tampered_since"), Some(cutoff));
    }

    #[tokio::test]
    async fn fail_next_short_circuits_the_next_call() {
        let m = MockEvidenceSource::new();
        m.fail_next("simulated DB down");
        let err = m.audit_log_tampered_since(Utc::now()).await.unwrap_err();
        assert!(matches!(err, EvidenceError::Unavailable(msg) if msg.contains("DB down")));
        // Subsequent calls succeed — fail_next is one-shot.
        assert!(m.audit_log_tampered_since(Utc::now()).await.is_ok());
    }
}
