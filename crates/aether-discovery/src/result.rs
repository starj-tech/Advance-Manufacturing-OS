use crate::probe::ProbeKind;
use crate::suggestion::SuggestedBinding;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VendorMetadata {
    /// Vendor display name (e.g. "Siemens", "Allen-Bradley", "Beckhoff").
    pub vendor: Option<String>,
    /// Product line (e.g. "S7-1500", "ControlLogix").
    pub model: Option<String>,
    /// Firmware revision string when the protocol exposes it.
    pub firmware: Option<String>,
    /// Free-form extras keyed by probe kind.
    pub extras: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    /// Stable identity for de-duplication across probes (typically the
    /// MAC address of the responder, or hash of host:port if MAC is
    /// unavailable).
    pub fingerprint: String,

    pub host: String,
    pub port: u16,
    pub probe: ProbeKind,
    pub metadata: VendorMetadata,
    pub suggested_bindings: Vec<SuggestedBinding>,
    pub discovered_at: DateTime<Utc>,
}

impl DiscoveredDevice {
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
