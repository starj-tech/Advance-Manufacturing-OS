//! Smart Interlock — physical lock-out of unqualified operators.
//!
//! The interlock subsystem sits between the user (via badge / passkey)
//! and the machine's safety relay. When a user attempts to bring up a
//! machine the controller:
//!
//!   1. Resolves the user's certifications from `aether-db` (mirrored
//!      from Supabase `certifications` table).
//!   2. Looks up the machine's `required_certifications`.
//!   3. If any required cert is missing or expired → emits a `Verdict`
//!      of `Deny` and writes a Modbus / OPC-UA boolean = false to the
//!      machine's "permitted-to-run" coil, which is wired to the safety
//!      relay's enable input. The motor will physically not energize.
//!
//! ## Threat model
//! The interlock is enforced **at the machine**, not just in the UI.
//! Even if an attacker bypasses the AETHER-OS UI, the safety relay only
//! closes when the protocol bridge writes `true` to the permit coil.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Allow,
    /// User lacks one or more required certifications — listed by code.
    DenyMissingCert(Vec<String>),
    /// User has the cert but it expired before `as_of`.
    DenyExpiredCert(Vec<String>),
    /// User is in lockout (post-incident review, suspended).
    DenyLockout(String),
    /// Machine itself reports a fault that prevents permit.
    DenyMachineFault(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certification {
    pub user_id: Uuid,
    pub code: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

impl Certification {
    pub fn is_valid_at(&self, instant: DateTime<Utc>) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(exp) = self.expires_at {
            if instant >= exp {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnlockRequest {
    pub user_id: Uuid,
    pub machine_id: Uuid,
    pub user_certs: Vec<Certification>,
    pub required_certs: Vec<String>,
    pub user_lockout_reason: Option<String>,
    pub machine_fault: Option<String>,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum InterlockError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("permit write failed: {0}")]
    PermitWrite(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Pure logic: maps a request to a verdict. Side effects (writing the
/// permit coil) are the controller's job.
pub fn evaluate(req: &UnlockRequest) -> Verdict {
    if let Some(reason) = &req.user_lockout_reason {
        return Verdict::DenyLockout(reason.clone());
    }
    if let Some(fault) = &req.machine_fault {
        return Verdict::DenyMachineFault(fault.clone());
    }

    let mut missing: Vec<String> = vec![];
    let mut expired: Vec<String> = vec![];

    for code in &req.required_certs {
        match req.user_certs.iter().find(|c| &c.code == code) {
            None => missing.push(code.clone()),
            Some(c) if !c.is_valid_at(req.as_of) => expired.push(code.clone()),
            Some(_) => {}
        }
    }

    if !missing.is_empty() {
        return Verdict::DenyMissingCert(missing);
    }
    if !expired.is_empty() {
        return Verdict::DenyExpiredCert(expired);
    }

    Verdict::Allow
}

#[async_trait::async_trait]
pub trait InterlockController: Send + Sync {
    /// Run `evaluate`, then translate to a permit signal on the machine.
    /// Implementations write to the safety relay's enable coil via the
    /// configured protocol bridge (OPC-UA / Modbus). Verdict::Allow
    /// → permit=true; any Deny → permit=false + audit log entry.
    async fn request_unlock(&self, req: &UnlockRequest) -> Result<Verdict, InterlockError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn cert(code: &str, expires_in_days: i64, revoked: bool) -> Certification {
        let now = Utc::now();
        Certification {
            user_id: Uuid::nil(),
            code: code.into(),
            issued_at: now - Duration::days(30),
            expires_at: Some(now + Duration::days(expires_in_days)),
            revoked,
        }
    }

    fn base_request() -> UnlockRequest {
        UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![],
            required_certs: vec!["lathe-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        }
    }

    #[test]
    fn allow_when_all_certs_valid() {
        let mut req = base_request();
        req.user_certs.push(cert("lathe-operator", 30, false));
        assert_eq!(evaluate(&req), Verdict::Allow);
    }

    #[test]
    fn deny_when_cert_missing() {
        let req = base_request();
        let v = evaluate(&req);
        assert!(
            matches!(v, Verdict::DenyMissingCert(ref m) if m == &vec!["lathe-operator".to_string()])
        );
    }

    #[test]
    fn deny_when_cert_expired() {
        let mut req = base_request();
        req.user_certs.push(cert("lathe-operator", -1, false));
        let v = evaluate(&req);
        assert!(
            matches!(v, Verdict::DenyExpiredCert(ref m) if m == &vec!["lathe-operator".to_string()])
        );
    }

    #[test]
    fn deny_when_cert_revoked_treated_as_expired() {
        let mut req = base_request();
        req.user_certs.push(cert("lathe-operator", 30, true));
        let v = evaluate(&req);
        assert!(matches!(v, Verdict::DenyExpiredCert(_)));
    }

    #[test]
    fn lockout_overrides_cert_check() {
        let mut req = base_request();
        req.user_certs.push(cert("lathe-operator", 30, false));
        req.user_lockout_reason = Some("post-incident review".into());
        assert!(matches!(evaluate(&req), Verdict::DenyLockout(_)));
    }

    #[test]
    fn machine_fault_overrides_cert_check() {
        let mut req = base_request();
        req.user_certs.push(cert("lathe-operator", 30, false));
        req.machine_fault = Some("e-stop pressed".into());
        assert!(matches!(evaluate(&req), Verdict::DenyMachineFault(_)));
    }
}
