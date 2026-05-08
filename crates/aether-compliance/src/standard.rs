use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StandardKind {
    /// Voluntary international standard (ISO, IEC, ASTM…).
    International,
    /// Industry-specific, often layered on top of ISO 9001.
    Industry,
    /// Government / supranational regulation with legal weight.
    Regulation,
    /// Local national standard (BPOM, SNI, JIS…).
    National,
}

/// Catalog entry — Serialize-only so we keep `&'static str` and
/// reconstruct from the code-defined catalog when reading reports.
#[derive(Clone, Debug, Serialize)]
pub struct ComplianceStandard {
    pub slug: &'static str,
    pub display: &'static str,
    pub jurisdiction: &'static str,
    pub kind: StandardKind,
    pub summary: &'static str,
    pub control_point_ids: &'static [&'static str],
}
