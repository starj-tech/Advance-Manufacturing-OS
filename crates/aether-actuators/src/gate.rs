//! `gate()` — the only public path to mint an [`crate::ActuatorPermit`].
//!
//! ## What gate does
//! It calls [`aether_safety::interlock::evaluate`] on the caller's
//! `UnlockRequest`, and:
//!
//!   * If the verdict is `Allow` → mint a fresh single-use permit
//!     with the configured TTL and return it.
//!   * Any `Deny*` verdict → return `GateError::InterlockDenied` with
//!     the verdict attached. No permit is ever leaked.
//!
//! ## Why a free function, not a builder
//! The permit constructor is crate-private. A builder would either
//! re-expose the constructor (defeating the gate) or duplicate the
//! interlock call (drift hazard). One free function is the simplest
//! shape that's also the most auditable: any code path that creates
//! a permit grep-finds to exactly one call site here.

use crate::permit::{ActuatorPermit, PermitTtl, DEFAULT_PERMIT_TTL};
use aether_safety::interlock::{evaluate, UnlockRequest, Verdict};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GateError {
    /// Interlock evaluation returned a Deny variant. The full verdict
    /// is attached for the caller to surface (UI shows "missing cert
    /// X" or "machine in fault Y").
    #[error("interlock denied: {0:?}")]
    InterlockDenied(Verdict),
}

/// Mint a permit if and only if the interlock verdict is `Allow`.
/// Uses [`DEFAULT_PERMIT_TTL`]; callers needing a custom TTL go through
/// [`gate_with_ttl`].
pub fn gate(req: &UnlockRequest) -> Result<ActuatorPermit, GateError> {
    gate_with_ttl(req, DEFAULT_PERMIT_TTL)
}

/// Like [`gate`] but accepts a custom TTL. Production rarely needs
/// this — tests pin short TTLs to exercise expiry paths without
/// `sleep(5s)`.
pub fn gate_with_ttl(req: &UnlockRequest, ttl: PermitTtl) -> Result<ActuatorPermit, GateError> {
    match evaluate(req) {
        Verdict::Allow => Ok(ActuatorPermit::new_unchecked(ttl)),
        deny => Err(GateError::InterlockDenied(deny)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_safety::interlock::Certification;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn cert(code: &str, valid_for_days: i64) -> Certification {
        Certification {
            user_id: Uuid::nil(),
            code: code.into(),
            issued_at: Utc::now() - Duration::days(30),
            expires_at: Some(Utc::now() + Duration::days(valid_for_days)),
            revoked: false,
        }
    }

    fn base_request_with_valid_cert() -> UnlockRequest {
        UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![cert("robot-op", 30)],
            required_certs: vec!["robot-op".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        }
    }

    #[test]
    fn allow_verdict_mints_a_permit() {
        let permit = gate(&base_request_with_valid_cert()).unwrap();
        assert!(!permit.is_consumed());
        assert!(!permit.is_expired_at(Utc::now()));
    }

    #[test]
    fn missing_cert_returns_interlock_denied_without_permit() {
        // The headline property: a Deny verdict NEVER hands back a
        // permit — there's no fallback or "soft permit" path. If the
        // interlock says no, the dispatch surface is unreachable.
        let mut req = base_request_with_valid_cert();
        req.required_certs.push("welder-op".into()); // user lacks it
        let err = gate(&req).unwrap_err();
        assert!(
            matches!(err, GateError::InterlockDenied(Verdict::DenyMissingCert(ref m)) if m == &vec!["welder-op".to_string()])
        );
    }

    #[test]
    fn lockout_returns_interlock_denied() {
        let mut req = base_request_with_valid_cert();
        req.user_lockout_reason = Some("post-incident review".into());
        let err = gate(&req).unwrap_err();
        assert!(matches!(
            err,
            GateError::InterlockDenied(Verdict::DenyLockout(_))
        ));
    }

    #[test]
    fn machine_fault_returns_interlock_denied() {
        let mut req = base_request_with_valid_cert();
        req.machine_fault = Some("e-stop pressed".into());
        let err = gate(&req).unwrap_err();
        assert!(matches!(
            err,
            GateError::InterlockDenied(Verdict::DenyMachineFault(_))
        ));
    }

    #[test]
    fn expired_cert_returns_interlock_denied() {
        let mut req = base_request_with_valid_cert();
        req.user_certs[0].expires_at = Some(Utc::now() - Duration::days(1));
        let err = gate(&req).unwrap_err();
        assert!(matches!(
            err,
            GateError::InterlockDenied(Verdict::DenyExpiredCert(_))
        ));
    }

    #[test]
    fn custom_ttl_is_threaded_to_the_permit() {
        let req = base_request_with_valid_cert();
        let permit = gate_with_ttl(&req, PermitTtl(100)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(permit.is_expired_at(Utc::now()));
    }
}
