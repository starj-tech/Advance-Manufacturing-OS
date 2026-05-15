//! Scanner — CIDR-iterating, concurrency-bounded orchestrator that
//! drives every registered `DiscoveryProbe` across a host range and
//! returns deduplicated devices.
//!
//! ## Algorithm
//! 1. Parse the requested CIDR (default: loopback `127.0.0.1/32` so a
//!    misconfigured request can't accidentally hammer the local
//!    subnet).
//! 2. For each (host × probe) pair, spawn a task on a tokio JoinSet
//!    bounded by a Semaphore. The semaphore caps in-flight connects
//!    at `req.concurrency` — the right knob to tune for "fast enough
//!    on a /24" vs "not tripping the IDS".
//! 3. Collect results, dropping any `Ok(None)` (port closed) and any
//!    `Err` (transient network errors — the host can be re-scanned
//!    next pass).
//! 4. Dedup by `DiscoveredDevice::fingerprint`. With the current
//!    fingerprint shape (`{host}:{port}/{protocol}`), dedup only
//!    activates when the same host:port appears twice in one scan
//!    — defensive against the rare case where a custom-config'd CIDR
//!    overlaps with itself.
//!
//! ## Why JoinSet over FuturesUnordered
//! `JoinSet::spawn` runs each future on the tokio scheduler, so the
//! work happens in parallel across the runtime's worker threads. A
//! `FuturesUnordered` polled from a single task would serialize the
//! polls and starve the CPU on a thread-pool runtime even when the
//! semaphore permits more concurrency.

use crate::cidr::{expand_cidr, CidrError};
use crate::probe::{DiscoveryProbe, ProbeError};
use crate::result::DiscoveredDevice;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    /// CIDR range to scan (e.g. `192.168.1.0/24`). `None` falls back
    /// to the loopback host so a missing field never blasts the LAN.
    pub cidr: Option<String>,
    /// Per-host probe timeout. Defaults to 1500 ms.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum concurrent probes. Defaults to 64 — matches the
    /// PR #3 plan ("64-way concurrency on a /24").
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

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("cidr: {0}")]
    Cidr(#[from] CidrError),
    #[error("probe: {0}")]
    Probe(#[from] ProbeError),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Orchestrates a parallel scan across registered probes. Probes are
/// stored as `Arc<dyn DiscoveryProbe>` so the spawned tasks share
/// ownership without copying probe state.
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

    pub async fn scan(&self, req: ScanRequest) -> Result<Vec<DiscoveredDevice>, ScanError> {
        if req.concurrency == 0 {
            return Err(ScanError::InvalidRequest("concurrency must be > 0".into()));
        }
        if self.probes.is_empty() {
            return Ok(Vec::new());
        }

        let cidr_str = req.cidr.as_deref().unwrap_or("127.0.0.1/32");
        let hosts = expand_cidr(cidr_str)?;

        let sem = Arc::new(Semaphore::new(req.concurrency));
        let mut tasks: JoinSet<Option<DiscoveredDevice>> = JoinSet::new();

        for host in hosts {
            for probe in &self.probes {
                let probe = probe.clone();
                let sem = sem.clone();
                let host = host.to_string();
                tasks.spawn(async move {
                    // Permit acquired here so the bound applies to the
                    // probe call, not the spawn rate. `acquire_owned`
                    // would be cleaner but requires owning the
                    // semaphore; `acquire` returns a guard tied to the
                    // arc.
                    let _permit = sem.acquire().await.ok()?;
                    let port = probe.target_port();
                    // Coerce probe errors (timeout / network) to "no
                    // device here" — the scanner reports what's
                    // present, not what couldn't be reached. The
                    // next scan pass will retry.
                    probe.probe_host(&host, port).await.ok().flatten()
                });
            }
        }

        let mut devices: Vec<DiscoveredDevice> = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            // join_next() Err means the spawned task panicked. Skip
            // it — a panicking probe shouldn't block the rest of the
            // scan from completing.
            if let Ok(Some(device)) = joined {
                devices.push(device);
            }
        }

        // Stable dedup by fingerprint. Sort first because dedup_by
        // only collapses adjacent equal entries.
        devices.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
        devices.dedup_by(|a, b| a.fingerprint == b.fingerprint);
        Ok(devices)
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
    use crate::probe::{ModbusProbe, MqttProbe, OpcUaProbe, ProbeKind};
    use crate::result::{DiscoveredDevice, VendorMetadata};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::net::TcpListener;

    #[test]
    fn scanner_collects_probes() {
        let s = Scanner::new()
            .with_probe(Arc::new(OpcUaProbe::new()))
            .with_probe(Arc::new(MqttProbe::new()))
            .with_probe(Arc::new(ModbusProbe::new()));
        assert_eq!(s.probe_count(), 3);
    }

    #[test]
    fn scan_request_defaults_are_reasonable() {
        let r = ScanRequest::default();
        assert!(r.cidr.is_none());
        assert_eq!(r.timeout_ms, 1500);
        assert_eq!(r.concurrency, 64);
    }

    #[tokio::test]
    async fn empty_scanner_returns_no_devices() {
        // No probes registered → no work. Doesn't panic, doesn't
        // misinterpret the empty CIDR fallback as "scan everything".
        let s = Scanner::new();
        let devices = s.scan(ScanRequest::default()).await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn zero_concurrency_is_rejected() {
        // A Semaphore::new(0) would deadlock every probe; we reject
        // the request before touching the network.
        let s = Scanner::new().with_probe(Arc::new(OpcUaProbe::new()));
        let req = ScanRequest {
            concurrency: 0,
            ..Default::default()
        };
        let err = s.scan(req).await.unwrap_err();
        assert!(matches!(err, ScanError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn invalid_cidr_surfaces_typed_error() {
        let s = Scanner::new().with_probe(Arc::new(OpcUaProbe::new()));
        let req = ScanRequest {
            cidr: Some("not.an.ip".into()),
            ..Default::default()
        };
        let err = s.scan(req).await.unwrap_err();
        assert!(matches!(err, ScanError::Cidr(_)));
    }

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
    async fn scan_finds_a_live_host_with_real_probe() {
        // End-to-end: a TCP listener stands in for an OPC-UA server;
        // scanner against 127.0.0.1/32 with an OpcUaProbe-on-that-port
        // discovers it.
        let port = spawn_open_port().await;
        let s = Scanner::new().with_probe(Arc::new(
            OpcUaProbe::new()
                .with_port(port)
                .with_timeout(Duration::from_millis(500)),
        ));

        let req = ScanRequest {
            cidr: Some("127.0.0.1/32".into()),
            timeout_ms: 500,
            concurrency: 8,
        };
        let devices = s.scan(req).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].host, "127.0.0.1");
        assert_eq!(devices[0].port, port);
        assert_eq!(devices[0].probe, ProbeKind::OpcUa);
    }

    /// Scripted probe for orchestration tests — doesn't open sockets.
    /// Returns a canned device for any host matching `match_host`
    /// and `None` otherwise.
    struct ScriptedProbe {
        kind: ProbeKind,
        port: u16,
        match_host: &'static str,
        fingerprint_override: Option<&'static str>,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl DiscoveryProbe for ScriptedProbe {
        fn kind(&self) -> ProbeKind {
            self.kind
        }
        fn target_port(&self) -> u16 {
            self.port
        }
        async fn probe_host(
            &self,
            host: &str,
            port: u16,
        ) -> Result<Option<DiscoveredDevice>, ProbeError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if host != self.match_host {
                return Ok(None);
            }
            let fingerprint = self
                .fingerprint_override
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{host}:{port}/{}", self.kind.slug()));
            Ok(Some(DiscoveredDevice {
                fingerprint,
                host: host.to_string(),
                port,
                probe: self.kind,
                metadata: VendorMetadata {
                    vendor: Some("scripted".into()),
                    model: None,
                    firmware: None,
                    extras: serde_json::json!({}),
                },
                suggested_bindings: vec![],
                discovered_at: Utc::now(),
            }))
        }
    }

    #[tokio::test]
    async fn scanner_dispatches_every_probe_against_every_host() {
        // Sanity: with 4 hosts × 2 probes the scanner makes 8 calls.
        let calls_a = Arc::new(AtomicUsize::new(0));
        let calls_b = Arc::new(AtomicUsize::new(0));
        let probe_a = Arc::new(ScriptedProbe {
            kind: ProbeKind::OpcUa,
            port: 4840,
            match_host: "192.168.1.0", // matches network address only
            fingerprint_override: None,
            call_count: calls_a.clone(),
        });
        let probe_b = Arc::new(ScriptedProbe {
            kind: ProbeKind::Mqtt,
            port: 1883,
            match_host: "no-match",
            fingerprint_override: None,
            call_count: calls_b.clone(),
        });
        let s = Scanner::new().with_probe(probe_a).with_probe(probe_b);

        // /30 = 4 hosts.
        let req = ScanRequest {
            cidr: Some("192.168.1.0/30".into()),
            timeout_ms: 100,
            concurrency: 8,
        };
        s.scan(req).await.unwrap();
        assert_eq!(calls_a.load(Ordering::SeqCst), 4);
        assert_eq!(calls_b.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn dedup_collapses_identical_fingerprints() {
        // Two scripted probes both return the same fingerprint for
        // the same host. Without dedup the result would carry two
        // entries; the scanner collapses them to one.
        let calls = Arc::new(AtomicUsize::new(0));
        let probe_a = Arc::new(ScriptedProbe {
            kind: ProbeKind::OpcUa,
            port: 4840,
            match_host: "127.0.0.1",
            fingerprint_override: Some("shared:dup"),
            call_count: calls.clone(),
        });
        let probe_b = Arc::new(ScriptedProbe {
            kind: ProbeKind::Mqtt,
            port: 1883,
            match_host: "127.0.0.1",
            fingerprint_override: Some("shared:dup"),
            call_count: calls.clone(),
        });
        let s = Scanner::new().with_probe(probe_a).with_probe(probe_b);

        let req = ScanRequest {
            cidr: Some("127.0.0.1/32".into()),
            timeout_ms: 100,
            concurrency: 4,
        };
        let devices = s.scan(req).await.unwrap();
        assert_eq!(devices.len(), 1, "duplicate fingerprints collapsed");
        // But both probes DID run — dedup happens after collection.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn distinct_fingerprints_are_all_returned() {
        let calls = Arc::new(AtomicUsize::new(0));
        let probe_a = Arc::new(ScriptedProbe {
            kind: ProbeKind::OpcUa,
            port: 4840,
            match_host: "127.0.0.1",
            fingerprint_override: None,
            call_count: calls.clone(),
        });
        let probe_b = Arc::new(ScriptedProbe {
            kind: ProbeKind::Mqtt,
            port: 1883,
            match_host: "127.0.0.1",
            fingerprint_override: None,
            call_count: calls.clone(),
        });
        let s = Scanner::new().with_probe(probe_a).with_probe(probe_b);

        let req = ScanRequest {
            cidr: Some("127.0.0.1/32".into()),
            timeout_ms: 100,
            concurrency: 4,
        };
        let devices = s.scan(req).await.unwrap();
        // Same host, two protocols → two distinct fingerprints, both
        // surfaced. The operator sees both bindings for the same box.
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn results_are_sorted_by_fingerprint_for_stable_output() {
        // Stable ordering matters because the UI / CLI prints the
        // list and operators often diff successive scans. Sort by
        // fingerprint produces a deterministic order regardless of
        // task-scheduler timing.
        let calls = Arc::new(AtomicUsize::new(0));
        // Two probes whose fingerprints sort in a known order; we
        // pin the OPC-UA-on-a-fingerprint-that-sorts-second so the
        // assertion is meaningful even if the runtime schedules
        // the MQTT task to complete first.
        let probe_a = Arc::new(ScriptedProbe {
            kind: ProbeKind::OpcUa,
            port: 4840,
            match_host: "127.0.0.1",
            fingerprint_override: Some("zz-second"),
            call_count: calls.clone(),
        });
        let probe_b = Arc::new(ScriptedProbe {
            kind: ProbeKind::Mqtt,
            port: 1883,
            match_host: "127.0.0.1",
            fingerprint_override: Some("aa-first"),
            call_count: calls.clone(),
        });
        let s = Scanner::new().with_probe(probe_a).with_probe(probe_b);
        let req = ScanRequest {
            cidr: Some("127.0.0.1/32".into()),
            timeout_ms: 100,
            concurrency: 4,
        };
        let devices = s.scan(req).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].fingerprint, "aa-first");
        assert_eq!(devices[1].fingerprint, "zz-second");
    }

    #[tokio::test]
    async fn no_cidr_defaults_to_loopback_host() {
        // Safety: a missing field shouldn't accidentally scan the
        // entire LAN. Default falls to 127.0.0.1/32 — one host.
        let calls = Arc::new(AtomicUsize::new(0));
        let probe = Arc::new(ScriptedProbe {
            kind: ProbeKind::OpcUa,
            port: 4840,
            match_host: "127.0.0.1",
            fingerprint_override: None,
            call_count: calls.clone(),
        });
        let s = Scanner::new().with_probe(probe);
        s.scan(ScanRequest::default()).await.unwrap();
        // Exactly one probe call against one host.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
