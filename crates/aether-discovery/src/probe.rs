use crate::result::DiscoveredDevice;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeKind {
    OpcUa,
    Mqtt,
    Modbus,
    EthernetIp,
}

impl ProbeKind {
    pub fn default_port(&self) -> u16 {
        match self {
            ProbeKind::OpcUa => 4840,
            ProbeKind::Mqtt => 1883,
            ProbeKind::Modbus => 502,
            ProbeKind::EthernetIp => 44818,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("network: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

#[async_trait::async_trait]
pub trait DiscoveryProbe: Send + Sync {
    fn kind(&self) -> ProbeKind;

    /// Probe a single host:port and return a device descriptor if it
    /// responds with a recognizable handshake.
    async fn probe_host(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError>;
}

// --- OPC-UA ---

/// Probes UA discovery service on `:4840` and emits OPC-UA application
/// description data. Real impl wires `opcua::client::Client::find_servers`
/// in PR #3.
pub struct OpcUaProbe;

#[async_trait::async_trait]
impl DiscoveryProbe for OpcUaProbe {
    fn kind(&self) -> ProbeKind {
        ProbeKind::OpcUa
    }

    async fn probe_host(
        &self,
        _host: &str,
        _port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError> {
        Err(ProbeError::NotImplemented("OpcUaProbe wired in PR #3"))
    }
}

// --- MQTT (mDNS / port scan) ---

/// Tries to open an MQTT CONNECT against the host. Real impl uses
/// `rumqttc` to perform a clean CONNECT/DISCONNECT round-trip in PR #3.
pub struct MqttProbe;

#[async_trait::async_trait]
impl DiscoveryProbe for MqttProbe {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Mqtt
    }

    async fn probe_host(
        &self,
        _host: &str,
        _port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError> {
        Err(ProbeError::NotImplemented("MqttProbe wired in PR #3"))
    }
}

// --- Modbus TCP ---

/// Issues a Modbus "Read Device Identification" function code 0x2B/0x0E
/// to fingerprint the controller. Real impl in PR #3 once the modbus
/// crate lands.
pub struct ModbusProbe;

#[async_trait::async_trait]
impl DiscoveryProbe for ModbusProbe {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Modbus
    }

    async fn probe_host(
        &self,
        _host: &str,
        _port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError> {
        Err(ProbeError::NotImplemented("ModbusProbe wired in PR #3"))
    }
}

// --- EtherNet/IP (CIP List Identity) ---

/// Sends the CIP List Identity request (UDP port 44818) and parses the
/// vendor / product code response. Real impl in PR #3.
pub struct EthernetIpProbe;

#[async_trait::async_trait]
impl DiscoveryProbe for EthernetIpProbe {
    fn kind(&self) -> ProbeKind {
        ProbeKind::EthernetIp
    }

    async fn probe_host(
        &self,
        _host: &str,
        _port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError> {
        Err(ProbeError::NotImplemented("EthernetIpProbe wired in PR #3"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_match_iana() {
        assert_eq!(ProbeKind::OpcUa.default_port(), 4840);
        assert_eq!(ProbeKind::Mqtt.default_port(), 1883);
        assert_eq!(ProbeKind::Modbus.default_port(), 502);
        assert_eq!(ProbeKind::EthernetIp.default_port(), 44818);
    }
}
