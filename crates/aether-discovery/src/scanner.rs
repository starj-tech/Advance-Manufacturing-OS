use crate::probe::{DiscoveryProbe, ProbeError};
use crate::result::DiscoveredDevice;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    /// CIDR range to scan (e.g. `192.168.1.0/24`). Empty = scan the
    /// device's own subnet detected from the default route.
    pub cidr: Option<String>,
    /// Per-host probe timeout. Defaults to 1500 ms.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum concurrent probes. Defaults to 64.
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
}

fn default_timeout_ms() -> u64 {
    1500
}
fn default_concurrency() -> usize {
    64
}

impl Default for ScanRequest {
    fn default() -> Self {
        Self {
            cidr: None,
            timeout_ms: default_timeout_ms(),
            concurrency: default_concurrency(),
        }
    }
}

/// Orchestrates a parallel scan across registered probes.
///
/// Skeleton scope: shape only; the actual traversal lands in PR #3 when
/// real probe implementations exist.
pub struct Scanner {
    probes: Vec<Arc<dyn DiscoveryProbe>>,
}

impl Scanner {
    pub fn new() -> Self {
        Self { probes: vec![] }
    }

    pub fn with_probe(mut self, probe: Arc<dyn DiscoveryProbe>) -> Self {
        self.probes.push(probe);
        self
    }

    pub fn probe_count(&self) -> usize {
        self.probes.len()
    }

    pub async fn scan(&self, _req: ScanRequest) -> Result<Vec<DiscoveredDevice>, ProbeError> {
        // Real algorithm (PR #3):
        //   1. Resolve CIDR → host iterator
        //   2. For each host × probe, dispatch under semaphore
        //   3. Deduplicate by fingerprint
        //   4. Apply vendor templates → SuggestedBinding hints
        Err(ProbeError::NotImplemented(
            "Scanner::scan wired in PR #3 with the protocol bridges",
        ))
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ModbusProbe, MqttProbe, OpcUaProbe};

    #[test]
    fn scanner_collects_probes() {
        let s = Scanner::new()
            .with_probe(Arc::new(OpcUaProbe))
            .with_probe(Arc::new(MqttProbe))
            .with_probe(Arc::new(ModbusProbe));
        assert_eq!(s.probe_count(), 3);
    }

    #[test]
    fn scan_request_defaults_are_reasonable() {
        let r = ScanRequest::default();
        assert!(r.cidr.is_none());
        assert_eq!(r.timeout_ms, 1500);
        assert_eq!(r.concurrency, 64);
    }
}
