//! Instant Compliance Attestation.
//!
//! Maps tenant operations to a curated catalog of 21+ regulatory and
//! voluntary standards (ISO 9001, GDPR, BPOM, SNI, FDA 21 CFR Part 11,
//! IATF 16949, AS9100, …). Each `ComplianceStandard` is decomposed into
//! `ControlPoint`s; each control point has a `Probe` that gathers
//! evidence from the rest of the system (audit log, certifications,
//! interlock events, sync metadata, etc.) and returns a `Verdict`.
//!
//! The aggregated `ComplianceReport` is timestamped, signed by the
//! cloud Edge Function `compliance-attest`, and persisted into
//! `compliance_reports` for proof.
//!
//! ## Skeleton scope
//! Standards catalog + control point shape + verdict types + a sample
//! probe trait. Real probes (and the LLM-assisted "explain why this
//! failed" surface) ship in PR #6.

pub mod audit_immutable_probe;
pub mod breach_notification_probe;
pub mod catalog;
pub mod cold_chain_probe;
pub mod control;
pub mod encryption_probe;
pub mod evidence;
pub mod hashchain;
pub mod incident_log_probe;
pub mod key_rotation_probe;
pub mod periodic_review_probe;
pub mod report;
pub mod runner;
pub mod standard;

pub use audit_immutable_probe::AuditTrailImmutableProbe;
pub use breach_notification_probe::BreachNotificationProbe;
pub use catalog::CATALOG;
pub use cold_chain_probe::ColdChainProbe;
pub use control::{ControlPoint, Probe, ProbeError, Verdict};
pub use encryption_probe::EncryptionAtRestProbe;
pub use evidence::{EvidenceError, EvidenceSource, MockEvidenceSource, OpenIncident};
pub use hashchain::{
    ChainError as HashChainError, EvidenceHashChain, EvidenceLink, GENESIS_HASH, HASH_LEN,
};
pub use incident_log_probe::IncidentLogProbe;
pub use key_rotation_probe::{KeyRotationProbe, DEFAULT_MAX_AGE_DAYS as KEY_ROTATION_MAX_AGE_DAYS};
pub use periodic_review_probe::{PeriodicReviewProbe, ReviewKind};
pub use report::{ChainError, ChainedVerdict, ComplianceReport, ControlVerdict, OverallStatus};
pub use runner::ProbeRunner;
pub use standard::{ComplianceStandard, StandardKind};
