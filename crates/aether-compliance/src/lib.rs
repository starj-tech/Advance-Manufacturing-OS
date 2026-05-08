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

pub mod catalog;
pub mod control;
pub mod report;
pub mod standard;

pub use catalog::CATALOG;
pub use control::{ControlPoint, Probe, ProbeError, Verdict};
pub use report::{ComplianceReport, OverallStatus};
pub use standard::{ComplianceStandard, StandardKind};
