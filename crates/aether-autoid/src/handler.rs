//! Scan handler trait — bridges raw decoded payloads to
//! application actions (badge unlock, lot consumption, job
//! dispatch, …).
//!
//! ## Why the indirection
//! The `Scanner` trait (V11) reads bytes off hardware. What those
//! bytes MEAN is application-specific: a 13-byte RFID UID is a
//! badge to the access-control flow but a tool ID to the
//! changeover wizard. Forcing the scanner impl to know the
//! workflow would tangle hardware + business logic. `ScanHandler`
//! is the thin seam that the pipeline calls between "we have a
//! decoded payload" and "we recorded an outcome."
//!
//! ## What a handler MUST do
//! Return a [`HandlerOutcome`] for every successful payload —
//! including the "I don't recognize this" case. Outcomes are
//! typed (`Acknowledge` / `Reject` / `Trigger`) so the pipeline
//! ledger can summarize without re-parsing free-form strings.
//! Only return [`HandlerError`] for INFRASTRUCTURE failures: the
//! cert-store is unreachable, the lot-database returned a 5xx,
//! etc. A scan that resolves to "this UID is not on the
//! allowlist" is `Ok(HandlerOutcome { action: Reject, … })`, NOT
//! an error.

use crate::scan::ScannedPayload;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What the handler decided to do with this scan. The pipeline
/// records this verbatim into the ledger; downstream code
/// (workflow router, UI) consumes the typed variant rather than
/// re-parsing a free-form string.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum HandlerAction {
    /// Scan recognized and accepted. No further action required
    /// from the pipeline; the handler has already side-effected
    /// whatever it needs (e.g. flipped an interlock permit).
    Acknowledge,
    /// Scan recognized but explicitly rejected. `reason` is the
    /// operator-facing string ("badge not on allowlist", "lot
    /// already consumed", "expired QC token"). NOT an error —
    /// the pipeline still writes a ledger row.
    Reject { reason: String },
    /// Scan recognized and routed to a downstream system. `target`
    /// names the system (e.g. "agv-dispatch", "interlock-eval");
    /// `ref_id` carries the correlation id the downstream returned
    /// (route id, evaluation id) for forensic linkage.
    Trigger {
        target: String,
        ref_id: Option<String>,
    },
}

impl HandlerAction {
    /// Kebab-case slug for the ledger column. Pinned by test so a
    /// rename in code is a schema migration.
    pub fn slug(&self) -> &'static str {
        match self {
            HandlerAction::Acknowledge => "acknowledge",
            HandlerAction::Reject { .. } => "reject",
            HandlerAction::Trigger { .. } => "trigger",
        }
    }
}

/// Handler verdict. `summary` is the one-line audit string;
/// `detail` carries structured handler-specific data (badge
/// metadata, lot quantities, dispatched route info).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandlerOutcome {
    pub action: HandlerAction,
    pub summary: String,
    pub detail: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum HandlerError {
    /// Infrastructure dependency unreachable (cert store DB,
    /// dispatch service, …). Retryable — the scan itself was
    /// fine; the handler couldn't complete its lookup.
    #[error("infrastructure: {0}")]
    Infrastructure(String),
    /// Handler refuses the payload class entirely (e.g. a badge
    /// handler received a GS1-128 barcode). Distinct from
    /// `Reject` because the scan is fundamentally wrong-shaped —
    /// the operator should be told "scan a badge, not a lot."
    #[error("unsupported class: {0}")]
    UnsupportedClass(&'static str),
    /// Payload bytes were the right shape but couldn't be parsed
    /// (e.g. GS1-128 missing the AI prefix, NDEF record
    /// truncated). The scanner produced bytes but the handler
    /// can't interpret them.
    #[error("malformed payload: {0}")]
    MalformedPayload(String),
}

#[async_trait]
pub trait ScanHandler: Send + Sync {
    /// Short kebab-case handler identifier, logged into every
    /// `ScanEvent` row so an auditor can answer "which handler
    /// processed this scan." Names should be stable across
    /// versions — they're part of the audit contract.
    fn handler_name(&self) -> &'static str;

    /// Process a decoded payload. Implementations MUST return
    /// `Ok(HandlerOutcome)` for every recognized payload —
    /// including explicit rejections. `Err(HandlerError)` is
    /// reserved for infrastructure failures and shape mismatches.
    async fn handle(&self, payload: &ScannedPayload) -> Result<HandlerOutcome, HandlerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_slug_is_kebab_case() {
        assert_eq!(HandlerAction::Acknowledge.slug(), "acknowledge");
        assert_eq!(
            HandlerAction::Reject { reason: "x".into() }.slug(),
            "reject"
        );
        assert_eq!(
            HandlerAction::Trigger {
                target: "x".into(),
                ref_id: None
            }
            .slug(),
            "trigger"
        );
    }

    #[test]
    fn handler_action_serde_round_trip_includes_tag() {
        // Pin wire format — Edge Functions pattern-match on
        // `action` so a tag rename is cross-stack breakage.
        let cases = [
            (
                HandlerAction::Acknowledge,
                serde_json::json!({"action": "acknowledge"}),
            ),
            (
                HandlerAction::Reject {
                    reason: "expired".into(),
                },
                serde_json::json!({"action": "reject", "reason": "expired"}),
            ),
            (
                HandlerAction::Trigger {
                    target: "agv-dispatch".into(),
                    ref_id: Some("route-7".into()),
                },
                serde_json::json!({
                    "action": "trigger",
                    "target": "agv-dispatch",
                    "ref_id": "route-7"
                }),
            ),
        ];
        for (action, expected) in cases {
            let v = serde_json::to_value(&action).unwrap();
            assert_eq!(v, expected);
            let back: HandlerAction = serde_json::from_value(v).unwrap();
            assert_eq!(back, action);
        }
    }
}
