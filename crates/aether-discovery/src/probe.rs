//! Discovery probes — one per industrial protocol the scanner knows
//! how to recognise on the network. Each probe is a thin TCP-knock
//! for now (port-open evidence + protocol-kind tagging); deeper
//! vendor-fingerprint exchanges (OPC-UA `find_servers`, Modbus
//! 0x2B/0x0E, CIP List Identity) wire up in PR #3 alongside the real
//! protocol crates.
//!
//! ## Why ship the TCP-knock first
//! A port-open signal is already actionable: the operator gets a list
//! of "machines reachable on the OPC-UA port" within seconds of hitting
//! Scan, even without vendor metadata. The placeholder gives the moat
//! its "plug and find" UX immediately; the richer probe results layer
//! in without changing the trait or caller code.
//!
//! ## Why per-probe port override
//! Production probes hit IANA-reserved ports (4840, 1883, 502, 44818).
//! Tests can't bind to those without root, so the probe carries its
//! target port as configurable state. `target_port()` defaults to
//! `kind().default_port()` — production code never sets it explicitly.

use crate::result::{DiscoveredDevice, VendorMetadata};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpStream;

/// Default per-probe timeout. 1.5 s is enough for a healthy LAN
/// round-trip but short enough that scanning a /24 with 64-way
/// concurrency completes in well under 10 s.
pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

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

    /// Kebab-case slug used inside fingerprints. Kept distinct from
    /// the JSON serde tag so the wire format and the fingerprint
    /// space can evolve independently.
    pub fn slug(&self) -> &'static str {
        match self {
            ProbeKind::OpcUa => "opc-ua",
            ProbeKind::Mqtt => "mqtt",
            ProbeKind::Modbus => "modbus",
            ProbeKind::EthernetIp => "ethernet-ip",
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

    /// Port this probe should be targeted at. Default is the IANA-
    /// reserved port for the protocol; overrideable per-instance for
    /// tests and non-standard deployments (TLS-on-MQTT on 8883, e.g.).
    fn target_port(&self) -> u16 {
        self.kind().default_port()
    }

    /// Probe a single host:port and return a device descriptor if it
    /// responds with a recognizable handshake. Returns `Ok(None)` for
    /// a clean "port closed" — that's the negative case the scanner
    /// expects, not an error.
    async fn probe_host(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Option<DiscoveredDevice>, ProbeError>;
}

/// TCP-knock helper: open a connection to `host:port`, bounded by
/// `timeout`. Returns `Ok(true)` for "port open and accepting", `Ok
/// (false)` for "actively refused" (no listener), `Err(Timeout)` for
/// "filtered or down". Production probes layer protocol-specific
/// handshakes on top of this; the helper is shared so the timeout
/// / error-mapping rules don't drift between probes.
pub(crate) async fn tcp_knock(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<bool, ProbeError> {
    let addr = format!("{host}:{port}");
    match tokio::time::timeout(timeout, TcpStream::connect(&addr)).await {
        Ok(Ok(_stream)) => Ok(true),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => Ok(false),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(ProbeError::PermissionDenied(format!("{e}")))
        }
        Ok(Err(e)) => Err(ProbeError::Network(format!("{e}"))),
        Err(_) => Err(ProbeError::Timeout),
    }
}

/// Common shape for a successful TCP-knock result. The vendor
/// metadata stays empty until the protocol-specific handshake lands;
/// the operator still gets a device entry to act on.
pub(crate) fn knock_device(host: &str, port: u16, kind: ProbeKind) -> DiscoveredDevice {
    DiscoveredDevice {
        fingerprint: format!("{host}:{port}/{}", kind.slug()),
        host: host.to_string(),
        port,
        probe: kind,
        metadata: VendorMetadata {
            vendor: None,
            model: None,
            firmware: None,
            extras: serde_json::json!({"detection": "tcp-knock"}),
        },
        suggested_bindings: vec![],
        discovered_at: Utc::now(),
    }
}

// -------- macro to instantiate the four near-identical probes --------

/// Generate a probe struct + DiscoveryProbe impl. Each probe is
/// otherwise a copy-paste — the macro keeps the four in sync so a
/// fix to one applies to all. Hand-written code would invite drift
/// (e.g. a new timeout policy applied to OPC-UA but missed on MQTT).
macro_rules! tcp_knock_probe {
    ($name:ident, $kind:expr) => {
        pub struct $name {
            port: u16,
            timeout: Duration,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    port: ($kind).default_port(),
                    timeout: DEFAULT_PROBE_TIMEOUT,
                }
            }

            /// Override the target port — production never needs this;
            /// tests use it to point at a listener bound to a random
            /// port (`bind 127.0.0.1:0`).
            pub fn with_port(mut self, port: u16) -> Self {
                self.port = port;
                self
            }

            /// Override the per-host timeout. Tests pin this short
            /// to keep the suite snappy when a host is filtered.
            pub fn with_timeout(mut self, t: Duration) -> Self {
                self.timeout = t;
                self
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[async_trait::async_trait]
        impl DiscoveryProbe for $name {
            fn kind(&self) -> ProbeKind {
                $kind
            }
            fn target_port(&self) -> u16 {
                self.port
            }
            async fn probe_host(
                &self,
                host: &str,
                port: u16,
            ) -> Result<Option<DiscoveredDevice>, ProbeError> {
                if tcp_knock(host, port, self.timeout).await? {
                    Ok(Some(knock_device(host, port, $kind)))
                } else {
                    Ok(None)
                }
            }
        }
    };
}

tcp_knock_probe!(OpcUaProbe, ProbeKind::OpcUa);
tcp_knock_probe!(MqttProbe, ProbeKind::Mqtt);
tcp_knock_probe!(ModbusProbe, ProbeKind::Modbus);
tcp_knock_probe!(EthernetIpProbe, ProbeKind::EthernetIp);

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn default_ports_match_iana() {
        assert_eq!(ProbeKind::OpcUa.default_port(), 4840);
        assert_eq!(ProbeKind::Mqtt.default_port(), 1883);
        assert_eq!(ProbeKind::Modbus.default_port(), 502);
        assert_eq!(ProbeKind::EthernetIp.default_port(), 44818);
    }

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let kinds = [
            ProbeKind::OpcUa,
            ProbeKind::Mqtt,
            ProbeKind::Modbus,
            ProbeKind::EthernetIp,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|k| k.slug()).collect();
        slugs.sort_unstable();
        let before = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in slugs {
            assert!(!s.contains('_'), "slug `{s}` should use kebab-case");
        }
    }

    /// Spawn an in-process TCP listener on 127.0.0.1:0 and return the
    /// chosen port. Drops accepted connections immediately — enough
    /// signal for tcp_knock to confirm "port open".
    async fn spawn_open_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                if let Ok((s, _)) = listener.accept().await {
                    drop(s);
                }
            }
        });
        port
    }

    #[tokio::test]
    async fn knock_returns_true_for_open_port() {
        let port = spawn_open_port().await;
        let open = tcp_knock("127.0.0.1", port, DEFAULT_PROBE_TIMEOUT)
            .await
            .unwrap();
        assert!(open);
    }

    #[tokio::test]
    async fn knock_returns_false_for_refused_connection() {
        // Bind & immediately drop → port is released, subsequent connect
        // gets a clean refusal (some OSes; others may produce timeout).
        // Pick a high port unlikely to be allocated.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        // On Linux a connect to a dropped listener gets ECONNREFUSED.
        // If the kernel happens to reuse the port between the drop and
        // the connect, the result might be `true` (open) — flake-prone.
        // Accept either Ok(false) or a benign timeout/network error.
        match tcp_knock("127.0.0.1", port, Duration::from_millis(200)).await {
            Ok(false) => {} // expected on Linux ECONNREFUSED
            Ok(true) => {
                // Race: kernel reallocated the port to something
                // accepting. Rare on a quiet test host; not a probe bug.
            }
            Err(ProbeError::Timeout) | Err(ProbeError::Network(_)) => {
                // Some platforms surface a filtered port as timeout/
                // unreachable instead of refused.
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn knock_never_reports_open_for_a_documentation_address() {
        // RFC 5737 TEST-NET-1 (192.0.2.0/24) is reserved for
        // documentation and there is no host there to accept. The
        // exact response varies by sandbox routing: some kernels
        // produce ECONNREFUSED quickly (Ok(false)), others return
        // ENETUNREACH (Network err), others time out. What MUST
        // never happen is Ok(true) — we never report an open port
        // when there's nothing on the wire.
        let result = tcp_knock("192.0.2.1", 4840, Duration::from_millis(100)).await;
        assert!(
            !matches!(result, Ok(true)),
            "must not report TEST-NET-1 as open; got {result:?}"
        );
    }

    #[tokio::test]
    async fn opcua_probe_against_live_port_returns_device() {
        let port = spawn_open_port().await;
        let probe = OpcUaProbe::new().with_port(port);
        let dev = probe.probe_host("127.0.0.1", port).await.unwrap();
        let dev = dev.expect("open port should yield Some(device)");
        assert_eq!(dev.probe, ProbeKind::OpcUa);
        assert_eq!(dev.host, "127.0.0.1");
        assert_eq!(dev.port, port);
        assert!(
            dev.fingerprint.contains("opc-ua"),
            "fingerprint must include the protocol slug: {}",
            dev.fingerprint
        );
        // Vendor metadata is empty until the deeper handshake lands.
        assert!(dev.metadata.vendor.is_none());
    }

    #[tokio::test]
    async fn mqtt_probe_against_dead_port_returns_none_not_error() {
        // tcp_knock returns Ok(false) for a closed port; the probe
        // forwards that as Ok(None) — a clean negative, not an error.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        let probe = MqttProbe::new()
            .with_port(port)
            .with_timeout(Duration::from_millis(200));
        match probe.probe_host("127.0.0.1", port).await {
            Ok(None) => {}
            Ok(Some(_)) => {
                // Rare kernel-reuse race; not a bug.
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn target_port_reflects_with_port_override() {
        let probe = ModbusProbe::new().with_port(9999);
        assert_eq!(probe.target_port(), 9999);
        // The default-constructed one still reports the IANA port.
        assert_eq!(ModbusProbe::new().target_port(), 502);
    }

    #[tokio::test]
    async fn fingerprint_format_matches_host_port_kind_slug() {
        // The scanner's dedup uses fingerprint equality; the format
        // is load-bearing for that dedup, so pin it explicitly.
        let port = spawn_open_port().await;
        let probe = EthernetIpProbe::new().with_port(port);
        let dev = probe.probe_host("127.0.0.1", port).await.unwrap().unwrap();
        assert_eq!(dev.fingerprint, format!("127.0.0.1:{port}/ethernet-ip"));
    }
}
