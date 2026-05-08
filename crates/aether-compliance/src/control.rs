use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Probe ran and the control is satisfied.
    Pass,
    /// Probe ran but the control is failing.
    Fail,
    /// Probe couldn't reach a verdict (missing data, dependent system down).
    NotApplicable,
    /// Manual review required (probe is rule-of-thumb, not deterministic).
    NeedsReview,
}

/// Catalog entry — Serialize-only so `&'static str` is preserved.
#[derive(Clone, Debug, Serialize)]
pub struct ControlPoint {
    pub id: &'static str,
    pub display: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("dependency missing: {0}")]
    DependencyMissing(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// A probe inspects the live system and returns a verdict for a single
/// control point. Probes are pure with respect to time — given the same
/// snapshot, they MUST return the same verdict, so every report is
/// reproducible from the audit log + DB at `as_of`.
#[async_trait::async_trait]
pub trait Probe: Send + Sync {
    fn control_point(&self) -> &ControlPoint;
    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError>;
}

/// Static catalog of control points referenced by the standards table.
/// New IDs added here flow into `aether-compliance` reports automatically.
pub const CONTROL_POINTS: &[ControlPoint] = &[
    // ISO 9001 QMS
    ControlPoint {
        id: "qms-document-control",
        display: "Document control",
        description: "All controlled documents have a current version, owner, and review date",
    },
    ControlPoint {
        id: "qms-mgmt-review",
        display: "Management review",
        description: "Periodic management review meetings with action items",
    },
    ControlPoint {
        id: "qms-internal-audit",
        display: "Internal audit",
        description: "Annual internal audit cycle covering every QMS clause",
    },
    ControlPoint {
        id: "qms-non-conformance",
        display: "Non-conformance",
        description: "NC raised, dispositioned, and closed inside SLA",
    },
    ControlPoint {
        id: "qms-capa",
        display: "CAPA closure",
        description: "Corrective/preventive actions closed with effectiveness check",
    },
    // Environmental
    ControlPoint {
        id: "env-aspects-register",
        display: "Environmental aspects",
        description: "Aspects identified, scored, and reviewed annually",
    },
    ControlPoint {
        id: "env-emissions-tracking",
        display: "Emissions tracking",
        description: "Continuous capture of GHG, VOC, and effluent",
    },
    // InfoSec
    ControlPoint {
        id: "is-access-review",
        display: "Access review",
        description: "Quarterly review of user access to sensitive resources",
    },
    ControlPoint {
        id: "is-incident-log",
        display: "Incident log",
        description: "Security incidents logged with response timeline",
    },
    ControlPoint {
        id: "is-encryption-at-rest",
        display: "Encryption at rest",
        description: "Sensitive columns encrypted client-side via aether-crypto",
    },
    ControlPoint {
        id: "is-key-rotation",
        display: "Key rotation",
        description: "DEKs rotated quarterly with epoch tracking",
    },
    // Food safety
    ControlPoint {
        id: "fs-haccp-plan",
        display: "HACCP plan",
        description: "Documented hazard analysis with critical control points",
    },
    ControlPoint {
        id: "fs-cold-chain",
        display: "Cold chain",
        description: "Cold-chain telemetry with no excursions in last 30 days",
    },
    ControlPoint {
        id: "fs-recall-test",
        display: "Recall test",
        description: "Mock recall completed within 2 hours",
    },
    ControlPoint {
        id: "fs-ccp-monitoring",
        display: "CCP monitoring",
        description: "Real-time CCP readings logged",
    },
    ControlPoint {
        id: "fs-corrective-action",
        display: "Corrective action",
        description: "Out-of-spec readings trigger documented corrective action",
    },
    ControlPoint {
        id: "fs-prp-management",
        display: "PRP management",
        description: "Pre-requisite programs documented and audited",
    },
    ControlPoint {
        id: "fs-food-fraud",
        display: "Food fraud",
        description: "Vulnerability assessment + mitigation plan",
    },
    // OHS
    ControlPoint {
        id: "ohs-incident-log",
        display: "Incident log",
        description: "Near-miss + injury reporting pipeline active",
    },
    ControlPoint {
        id: "ohs-jha",
        display: "Job hazard analysis",
        description: "JHA completed for every task type",
    },
    ControlPoint {
        id: "ohs-ppe-issuance",
        display: "PPE issuance",
        description: "Per-employee PPE issuance tracked",
    },
    // Medical devices
    ControlPoint {
        id: "md-design-controls",
        display: "Design controls",
        description: "DHF / DMR linkages complete",
    },
    ControlPoint {
        id: "md-udi-marking",
        display: "UDI marking",
        description: "Unique Device Identifier verified per unit",
    },
    ControlPoint {
        id: "md-dhf-link",
        display: "DHF link",
        description: "Design History File linked to current product",
    },
    ControlPoint {
        id: "md-mdr-reporting",
        display: "MDR reporting",
        description: "Medical Device Reports filed within timeline",
    },
    ControlPoint {
        id: "md-clinical-evaluation",
        display: "Clinical evaluation",
        description: "Clinical evaluation report current per MDR",
    },
    // Energy
    ControlPoint {
        id: "em-baseline",
        display: "Energy baseline",
        description: "Energy performance baseline established",
    },
    ControlPoint {
        id: "em-significant-uses",
        display: "Significant uses",
        description: "Significant Energy Uses documented",
    },
    // Automotive
    ControlPoint {
        id: "auto-ppap",
        display: "PPAP",
        description: "Production Part Approval Process pack on file",
    },
    ControlPoint {
        id: "auto-pfmea",
        display: "PFMEA",
        description: "Process FMEA reviewed and current",
    },
    ControlPoint {
        id: "auto-spc",
        display: "SPC",
        description: "Statistical Process Control on critical characteristics",
    },
    ControlPoint {
        id: "auto-msa",
        display: "MSA",
        description: "Measurement System Analysis (gauge R&R) within tolerance",
    },
    // Aerospace
    ControlPoint {
        id: "as-fai",
        display: "First Article Inspection",
        description: "AS9102 FAI completed for every new part",
    },
    ControlPoint {
        id: "as-counterfeit-parts",
        display: "Counterfeit parts",
        description: "Counterfeit-parts mitigation per AS5553",
    },
    ControlPoint {
        id: "as-config-mgmt",
        display: "Configuration management",
        description: "Configuration baseline tracked through change",
    },
    // Electronic records
    ControlPoint {
        id: "er-signature-binding",
        display: "Signature binding",
        description: "Electronic signatures bound to records, can't be detached",
    },
    ControlPoint {
        id: "er-audit-trail-immutable",
        display: "Audit trail",
        description: "Audit trail immutable; no UPDATE/DELETE on audit_log",
    },
    ControlPoint {
        id: "er-system-validation",
        display: "System validation",
        description: "IQ/OQ/PQ documentation current",
    },
    // Data protection
    ControlPoint {
        id: "dp-lawful-basis",
        display: "Lawful basis",
        description: "Lawful basis recorded for every PII processing purpose",
    },
    ControlPoint {
        id: "dp-dsar-pipeline",
        display: "DSAR pipeline",
        description: "Data Subject Access Requests fulfilled within 30 days",
    },
    ControlPoint {
        id: "dp-breach-notification",
        display: "Breach notification",
        description: "Breach notification within 72 hours",
    },
    ControlPoint {
        id: "dp-encryption-pii",
        display: "PII encryption",
        description: "PII columns encrypted client-side",
    },
    // Substances
    ControlPoint {
        id: "sub-decl-rohs",
        display: "RoHS declaration",
        description: "Material declarations from suppliers under RoHS limits",
    },
    ControlPoint {
        id: "sub-decl-reach",
        display: "REACH declaration",
        description: "Substances of Very High Concern declared",
    },
    ControlPoint {
        id: "sub-scip-submission",
        display: "SCIP submission",
        description: "SCIP database notification per REACH article 9",
    },
    // BPOM / SNI
    ControlPoint {
        id: "bpom-registration",
        display: "BPOM registration",
        description: "Active BPOM registration number per product",
    },
    ControlPoint {
        id: "bpom-gmp",
        display: "BPOM GMP/CPOTB",
        description: "GMP/CPOTB certificate current",
    },
    ControlPoint {
        id: "bpom-halal-traceability",
        display: "Halal traceability",
        description: "Halal certificate + ingredient traceability",
    },
    ControlPoint {
        id: "sni-cert-record",
        display: "SNI certificate",
        description: "SNI Wajib certificate on file",
    },
    ControlPoint {
        id: "sni-product-mark",
        display: "SNI mark",
        description: "SNI mark applied to in-scope products",
    },
    // FSC
    ControlPoint {
        id: "fsc-coc-claim",
        display: "FSC chain of custody",
        description: "Per-batch CoC claim recorded",
    },
    ControlPoint {
        id: "fsc-volume-summary",
        display: "FSC volume summary",
        description: "Annual FSC volume summary submitted",
    },
    // Cosmetics
    ControlPoint {
        id: "cos-batch-record",
        display: "Batch record",
        description: "Batch manufacturing record per ISO 22716",
    },
    ControlPoint {
        id: "cos-personnel-training",
        display: "Personnel training",
        description: "Annual GMP training certificate per operator",
    },
    // Battery
    ControlPoint {
        id: "bat-passport",
        display: "Battery passport",
        description: "Per-battery digital passport published",
    },
    ControlPoint {
        id: "bat-recycled-content",
        display: "Recycled content",
        description: "Recycled cobalt/lithium/nickel content declared",
    },
    ControlPoint {
        id: "bat-cfp",
        display: "Carbon footprint",
        description: "Per-cell carbon footprint calculated and disclosed",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CATALOG;
    use std::collections::HashSet;

    #[test]
    fn control_points_are_unique() {
        let mut seen: HashSet<&'static str> = HashSet::new();
        for cp in CONTROL_POINTS {
            assert!(seen.insert(cp.id), "duplicate control point id: {}", cp.id);
        }
    }

    #[test]
    fn every_standard_references_known_control_points() {
        let known: HashSet<&'static str> = CONTROL_POINTS.iter().map(|c| c.id).collect();
        for s in CATALOG {
            for cp in s.control_point_ids {
                assert!(
                    known.contains(*cp),
                    "{} references unknown control {}",
                    s.slug,
                    cp
                );
            }
        }
    }
}
