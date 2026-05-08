use crate::standard::{ComplianceStandard, StandardKind};

/// Catalog of standards supported at launch. Each entry references
/// control point slugs defined in `control.rs`. Adding a standard means:
///   1. Drop a row here.
///   2. Add any new control point slugs to `CONTROL_POINTS`.
///   3. Add a `Probe` impl in PR #6.
pub const CATALOG: &[ComplianceStandard] = &[
    ComplianceStandard {
        slug: "iso-9001",
        display: "ISO 9001:2015 — Quality management",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "Process-based QMS framework applicable to any industry.",
        control_point_ids: &[
            "qms-document-control",
            "qms-mgmt-review",
            "qms-internal-audit",
            "qms-non-conformance",
            "qms-capa",
        ],
    },
    ComplianceStandard {
        slug: "iso-14001",
        display: "ISO 14001 — Environmental management",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "Environmental aspects, impacts, and continual improvement.",
        control_point_ids: &["env-aspects-register", "env-emissions-tracking"],
    },
    ComplianceStandard {
        slug: "iso-27001",
        display: "ISO 27001 — Information security",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "ISMS controls; data classification, access, and incident response.",
        control_point_ids: &[
            "is-access-review",
            "is-incident-log",
            "is-encryption-at-rest",
            "is-key-rotation",
        ],
    },
    ComplianceStandard {
        slug: "iso-22000",
        display: "ISO 22000 — Food safety",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "HACCP-aligned food safety management.",
        control_point_ids: &["fs-haccp-plan", "fs-cold-chain", "fs-recall-test"],
    },
    ComplianceStandard {
        slug: "iso-45001",
        display: "ISO 45001 — Occupational health & safety",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "OHS risk assessment, PPE, incident reporting.",
        control_point_ids: &["ohs-incident-log", "ohs-jha", "ohs-ppe-issuance"],
    },
    ComplianceStandard {
        slug: "iso-13485",
        display: "ISO 13485 — Medical devices QMS",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Medical device design, validation, traceability.",
        control_point_ids: &["md-design-controls", "md-udi-marking", "md-dhf-link"],
    },
    ComplianceStandard {
        slug: "iso-50001",
        display: "ISO 50001 — Energy management",
        jurisdiction: "International",
        kind: StandardKind::International,
        summary: "Energy performance baseline, monitoring, and improvement.",
        control_point_ids: &["em-baseline", "em-significant-uses"],
    },
    ComplianceStandard {
        slug: "iatf-16949",
        display: "IATF 16949 — Automotive QMS",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Automotive layer on ISO 9001: PPAP, FMEA, SPC, MSA.",
        control_point_ids: &["auto-ppap", "auto-pfmea", "auto-spc", "auto-msa"],
    },
    ComplianceStandard {
        slug: "as9100",
        display: "AS9100D — Aerospace QMS",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Aerospace QMS layered on ISO 9001 with FAI, counterfeit parts.",
        control_point_ids: &["as-fai", "as-counterfeit-parts", "as-config-mgmt"],
    },
    ComplianceStandard {
        slug: "fda-21-cfr-11",
        display: "FDA 21 CFR Part 11 — Electronic records",
        jurisdiction: "United States",
        kind: StandardKind::Regulation,
        summary: "Electronic signatures, audit trail integrity, system validation.",
        control_point_ids: &[
            "er-signature-binding",
            "er-audit-trail-immutable",
            "er-system-validation",
        ],
    },
    ComplianceStandard {
        slug: "fda-21-cfr-820",
        display: "FDA 21 CFR Part 820 — Quality System Regulation",
        jurisdiction: "United States",
        kind: StandardKind::Regulation,
        summary: "US medical device QSR.",
        control_point_ids: &["md-design-controls", "md-dhf-link", "md-mdr-reporting"],
    },
    ComplianceStandard {
        slug: "gdpr",
        display: "GDPR — General Data Protection Regulation",
        jurisdiction: "European Union",
        kind: StandardKind::Regulation,
        summary: "Personal data lawful basis, DSAR fulfillment, breach notification.",
        control_point_ids: &[
            "dp-lawful-basis",
            "dp-dsar-pipeline",
            "dp-breach-notification",
            "dp-encryption-pii",
        ],
    },
    ComplianceStandard {
        slug: "hipaa",
        display: "HIPAA — Health Insurance Portability & Accountability Act",
        jurisdiction: "United States",
        kind: StandardKind::Regulation,
        summary: "PHI safeguards: administrative, physical, technical.",
        control_point_ids: &["dp-encryption-pii", "is-access-review", "is-incident-log"],
    },
    ComplianceStandard {
        slug: "rohs",
        display: "RoHS — Restriction of Hazardous Substances",
        jurisdiction: "European Union",
        kind: StandardKind::Regulation,
        summary: "Restriction of Pb, Hg, Cd, Cr(VI), PBB, PBDE in electronics.",
        control_point_ids: &["sub-decl-rohs"],
    },
    ComplianceStandard {
        slug: "reach",
        display: "REACH — Registration, Evaluation, Authorisation of Chemicals",
        jurisdiction: "European Union",
        kind: StandardKind::Regulation,
        summary: "SVHC declarations, scip database submission.",
        control_point_ids: &["sub-decl-reach", "sub-scip-submission"],
    },
    ComplianceStandard {
        slug: "ce-mdr",
        display: "CE / EU MDR — Medical Device Regulation",
        jurisdiction: "European Union",
        kind: StandardKind::Regulation,
        summary: "EU medical device CE marking under regulation 2017/745.",
        control_point_ids: &[
            "md-design-controls",
            "md-udi-marking",
            "md-clinical-evaluation",
        ],
    },
    ComplianceStandard {
        slug: "bpom",
        display: "BPOM — Indonesian Food & Drug Authority",
        jurisdiction: "Indonesia",
        kind: StandardKind::National,
        summary: "Registration, GMP/CPOTB, halal & labeling rules.",
        control_point_ids: &["bpom-registration", "bpom-gmp", "bpom-halal-traceability"],
    },
    ComplianceStandard {
        slug: "sni",
        display: "SNI — Indonesian National Standard",
        jurisdiction: "Indonesia",
        kind: StandardKind::National,
        summary: "Mandatory product certification (SNI Wajib).",
        control_point_ids: &["sni-cert-record", "sni-product-mark"],
    },
    ComplianceStandard {
        slug: "fsc",
        display: "FSC — Forest Stewardship Council",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Chain of custody for sustainably sourced timber.",
        control_point_ids: &["fsc-coc-claim", "fsc-volume-summary"],
    },
    ComplianceStandard {
        slug: "haccp",
        display: "HACCP — Hazard Analysis & Critical Control Points",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Food safety hazard identification + CCP monitoring.",
        control_point_ids: &["fs-haccp-plan", "fs-ccp-monitoring", "fs-corrective-action"],
    },
    ComplianceStandard {
        slug: "fssc-22000",
        display: "FSSC 22000 — Food Safety System Certification",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "GFSI-recognized scheme combining ISO 22000 + sector PRPs.",
        control_point_ids: &["fs-haccp-plan", "fs-prp-management", "fs-food-fraud"],
    },
    ComplianceStandard {
        slug: "iso-22716",
        display: "ISO 22716 — Cosmetics GMP",
        jurisdiction: "International",
        kind: StandardKind::Industry,
        summary: "Cosmetics GMP for production, control, storage, shipment.",
        control_point_ids: &["cos-batch-record", "cos-personnel-training"],
    },
    ComplianceStandard {
        slug: "eu-battery-regulation",
        display: "EU Battery Regulation (2023/1542)",
        jurisdiction: "European Union",
        kind: StandardKind::Regulation,
        summary: "Battery passport, recycled content, carbon footprint, due diligence.",
        control_point_ids: &["bat-passport", "bat-recycled-content", "bat-cfp"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn slugs_are_unique() {
        let mut seen: HashSet<&'static str> = HashSet::new();
        for s in CATALOG {
            assert!(seen.insert(s.slug), "duplicate slug: {}", s.slug);
        }
    }

    #[test]
    fn at_least_21_standards() {
        // Marketing claim: "21 industries". The actual count of standards
        // can exceed that — they're not 1:1 with industries — but we
        // hard-floor the catalog so we never accidentally regress below.
        assert!(CATALOG.len() >= 21, "catalog has only {}", CATALOG.len());
    }

    #[test]
    fn control_point_ids_are_non_empty() {
        for s in CATALOG {
            assert!(
                !s.control_point_ids.is_empty(),
                "{} has no control points",
                s.slug
            );
        }
    }
}
