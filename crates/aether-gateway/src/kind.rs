//! `GatewayKind` — discriminator across the three gateway
//! classes we route differently.
//!
//! ## Why a typed enum
//! The three classes have wildly different latency profiles
//! and buffering requirements:
//!   * Industrial gateways are typically the bottleneck on a
//!     factory floor — they aggregate hundreds of serial
//!     devices and forward over a slow uplink. Buffer-heavy.
//!   * Edge nodes have compute headroom and can preprocess
//!     before forwarding. Buffer is local to compute output.
//!   * Protocol bridges are pure pass-through; buffering would
//!     introduce ordering anomalies the upstream broker
//!     doesn't expect.
//!
//! Routing decisions (sync cadence, healing policy) branch on
//! the kind, so a typed discriminator beats a string.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GatewayKind {
    /// Rugged DIN-rail gateway aggregating serial / 4-20 mA
    /// devices into IP. Moxa UC-8100, Advantech UNO, eWON Cosy.
    Industrial,
    /// Compute-capable edge node running pre-processing or ML
    /// inference. Siemens Industrial Edge, AWS IoT Greengrass,
    /// Azure IoT Edge.
    Edge,
    /// Pure transport translator (Modbus → MQTT, OPC-UA → REST).
    /// No local compute, minimal buffering.
    ProtocolBridge,
}

impl GatewayKind {
    /// Kebab-case slug for the audit ledger + binding TOML.
    /// Renaming a variant is a schema migration.
    pub fn slug(&self) -> &'static str {
        match self {
            GatewayKind::Industrial => "industrial",
            GatewayKind::Edge => "edge",
            GatewayKind::ProtocolBridge => "protocol-bridge",
        }
    }

    /// Whether this gateway class typically runs local compute.
    /// The sync layer uses this to decide whether to forward
    /// pre-aggregated rollups or raw samples — Industrial /
    /// ProtocolBridge get raw; Edge gets to roll up first.
    pub fn supports_edge_compute(&self) -> bool {
        matches!(self, GatewayKind::Edge)
    }

    /// Whether buffering is the expected mode. Protocol bridges
    /// pass through; the other two store-and-forward by default.
    /// The healing layer treats a buffered ProtocolBridge as a
    /// misconfiguration signal.
    pub fn buffers_by_default(&self) -> bool {
        matches!(self, GatewayKind::Industrial | GatewayKind::Edge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let kinds = [
            GatewayKind::Industrial,
            GatewayKind::Edge,
            GatewayKind::ProtocolBridge,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|k| k.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn protocol_bridge_slug_uses_dash() {
        let s = serde_json::to_string(&GatewayKind::ProtocolBridge).unwrap();
        assert_eq!(s, "\"protocol-bridge\"");
        let back: GatewayKind = serde_json::from_str("\"protocol-bridge\"").unwrap();
        assert_eq!(back, GatewayKind::ProtocolBridge);
    }

    #[test]
    fn edge_compute_capability_is_edge_kind_only() {
        assert!(!GatewayKind::Industrial.supports_edge_compute());
        assert!(GatewayKind::Edge.supports_edge_compute());
        assert!(!GatewayKind::ProtocolBridge.supports_edge_compute());
    }

    #[test]
    fn buffering_defaults_distinguish_protocol_bridge_from_others() {
        // Protocol bridges pass through; the others buffer by
        // default. The healing layer treats a buffered bridge
        // as a misconfiguration signal.
        assert!(GatewayKind::Industrial.buffers_by_default());
        assert!(GatewayKind::Edge.buffers_by_default());
        assert!(!GatewayKind::ProtocolBridge.buffers_by_default());
    }
}
