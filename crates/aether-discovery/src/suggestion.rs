use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// Vendor metadata + protocol signature both match a known template.
    High,
    /// Protocol responded but vendor template is heuristic.
    Medium,
    /// Probe responded but no template matched; user must map manually.
    Low,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuggestedBinding {
    /// Domain tag the user can accept (e.g. `press_01.temp`).
    pub tag: String,
    /// Protocol-specific source string (NodeId, MQTT topic, register #).
    pub source: String,
    pub unit: Option<String>,
    pub scale: Option<f64>,
    pub confidence: Confidence,
    /// Human-readable rationale ("matched Siemens S7-1500 PT100 template").
    pub rationale: String,
}
