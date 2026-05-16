//! Privacy classification per camera — drives the "encrypt before
//! storage?" decision at the pipeline layer.
//!
//! ## Three classes
//!
//! - [`PrivacyClass::Public`] — frames carry no PII or proprietary
//!   process detail. Examples: a line-side IR sensor view of a
//!   conveyor, a generic packaging-line camera with no human in
//!   shot. Stored in plaintext object storage; the audit row carries
//!   only the URL + hash.
//!
//! - [`PrivacyClass::OperatorOnly`] — frames may include operator
//!   faces, badge numbers, or other employee-identifying signals.
//!   Visible only to authenticated operators of the same tenant;
//!   the bytes themselves still go to object storage but with
//!   tenant-scoped RLS. **Not** encrypted at rest.
//!
//! - [`PrivacyClass::Sensitive`] — frames contain something the
//!   tenant has decided is high-impact: R&D process video, a
//!   recipe inspection, biometric capture. Bytes are sealed via
//!   `aether-crypto::envelope` before any durable write. Only
//!   client-side decryption by a key-holding device can recover
//!   the original.
//!
//! ## Why a closed enum rather than a string
//! The classification gates a load-bearing branch ("seal or not").
//! Adding a new class is a workspace-wide decision (every consumer
//! must match exhaustively) — that's the correct property. A string
//! would silently bucket misspellings (`"Sensetive"`) into the
//! permissive default and leak privacy.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrivacyClass {
    Public,
    OperatorOnly,
    Sensitive,
}

impl PrivacyClass {
    /// `true` if frames at this class MUST be sealed via
    /// `aether-crypto::envelope` before any durable storage write.
    /// The pipeline (V4) consults this immediately after a Capture
    /// dispatch and routes the bytes accordingly.
    pub fn requires_encryption(&self) -> bool {
        matches!(self, PrivacyClass::Sensitive)
    }

    /// `true` if the frame can be returned to any authenticated user
    /// in the tenant. `Sensitive` requires the requester to hold the
    /// per-tenant DEK; `OperatorOnly` requires operator role; `Public`
    /// is open to any tenant member.
    pub fn requires_operator_role(&self) -> bool {
        matches!(self, PrivacyClass::OperatorOnly | PrivacyClass::Sensitive)
    }

    /// Kebab-case slug — pinned for the audit log so wire format
    /// stays stable even when display names evolve.
    pub fn slug(&self) -> &'static str {
        match self {
            PrivacyClass::Public => "public",
            PrivacyClass::OperatorOnly => "operator-only",
            PrivacyClass::Sensitive => "sensitive",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_requires_encryption_others_do_not() {
        // The load-bearing branch: only `Sensitive` gets sealed.
        // If this test ever needs updating because a new class also
        // requires encryption, the encryption pipeline at the
        // call-site needs an updated review at the same time.
        assert!(PrivacyClass::Sensitive.requires_encryption());
        assert!(!PrivacyClass::OperatorOnly.requires_encryption());
        assert!(!PrivacyClass::Public.requires_encryption());
    }

    #[test]
    fn operator_role_required_for_non_public_classes() {
        assert!(!PrivacyClass::Public.requires_operator_role());
        assert!(PrivacyClass::OperatorOnly.requires_operator_role());
        assert!(PrivacyClass::Sensitive.requires_operator_role());
    }

    #[test]
    fn slugs_are_distinct_and_kebab_cased() {
        // Audit rows persist the slug; collisions or non-kebab
        // spelling would silently corrupt the audit schema.
        let classes = [
            PrivacyClass::Public,
            PrivacyClass::OperatorOnly,
            PrivacyClass::Sensitive,
        ];
        let mut slugs: Vec<&'static str> = classes.iter().map(|c| c.slug()).collect();
        slugs.sort_unstable();
        let before = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn serde_round_trip_uses_kebab_case() {
        // The JSON wire format is what an Edge Function will read;
        // pin it explicitly so the field name never drifts.
        let s = serde_json::to_string(&PrivacyClass::OperatorOnly).unwrap();
        assert_eq!(s, "\"operator-only\"");
        let back: PrivacyClass = serde_json::from_str(&s).unwrap();
        assert_eq!(back, PrivacyClass::OperatorOnly);
    }
}
