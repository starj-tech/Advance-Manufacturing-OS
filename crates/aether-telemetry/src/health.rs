//! `/health` and `/ready` HTTP endpoints for external uptime
//! monitors (BetterStack, Statuspage, internal Kubernetes
//! probes).
//!
//! ## Liveness vs. readiness
//! Two distinct concerns the convention separates:
//!
//!   * **`/health`** — "is the process alive?" Returns 200 if
//!     the binary is running and the event loop is responsive.
//!     A monitor watching only this endpoint sees "process
//!     crashed" but not "process is up but stuck on bad
//!     dependency."
//!
//!   * **`/ready`** — "is the process ready to take traffic?"
//!     Returns 200 only when every registered dependency
//!     check passes. A monitor watching this sees the same
//!     "crashed" signal AND degraded-mode signals (sync engine
//!     stuck, sqlite locked, telemetry sink in distress).
//!
//! Production load balancers / external monitors target
//! `/ready`; internal supervisor processes (systemd, k8s
//! kubelet) target `/health` for restart decisions.
//!
//! ## Why hyper directly instead of axum
//! The endpoint is two routes, no middleware, no extractors,
//! no body parsing. hyper alone keeps the dep footprint
//! minimal (a couple hundred KB) — axum would double that
//! for no gain on a surface this small.
//!
//! ## Registering dependency checks
//! The server starts with an empty checks list and is
//! "always ready." Callers register named async closures via
//! `HealthServer::add_check(name, future)`. A check returns
//! `Ok(())` for healthy or `Err(reason)` for degraded; the
//! `/ready` response body lists every failing check by name
//! so the monitor's incident description is self-explanatory.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

/// Default port for the health endpoint. 8088 chosen to
/// avoid colliding with the Tauri devserver (1420), Tauri
/// IPC (1430), or any common protocol probe ports the V9/V16
/// discovery layer targets.
pub const DEFAULT_HEALTH_PORT: u16 = 8088;

/// Per-check timeout. Health probes that hang would block
/// the responder; we cap them so a stuck dependency surfaces
/// as a failed check, not a hung endpoint.
pub const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of one named dependency check. The string field
/// describes the failure mode for the monitor's incident
/// log.
pub type CheckResult = Result<(), String>;

/// Dynamic-dispatch check function. Returns a boxed future
/// so the server can hold heterogeneous checks in one map.
pub type CheckFn = Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = CheckResult> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug, Error)]
pub enum HealthError {
    #[error("bind {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("server: {0}")]
    Server(String),
}

/// JSON body returned by `/health` and `/ready`.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct HealthBody {
    pub status: &'static str,
    pub at: DateTime<Utc>,
    /// Failing-check details — present only when status is
    /// `degraded`. Each entry is `{ "name": "...", "reason":
    /// "..." }`; the monitor renders this verbatim in the
    /// incident description.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<FailureDetail>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FailureDetail {
    pub name: String,
    pub reason: String,
}

/// Server handle. Cheap to clone; clones share the same
/// check registry so a check added on one clone is observed
/// by every active responder.
#[derive(Clone)]
pub struct HealthServer {
    checks: Arc<RwLock<HashMap<String, CheckFn>>>,
    check_timeout: Duration,
}

impl HealthServer {
    pub fn new() -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            check_timeout: DEFAULT_CHECK_TIMEOUT,
        }
    }

    pub fn with_check_timeout(mut self, timeout: Duration) -> Self {
        self.check_timeout = timeout;
        self
    }

    /// Register a dependency check under a stable name. Names
    /// are persisted into the `/ready` body so a check rename
    /// is a monitor-incident-log breakage; pick deliberate
    /// kebab-case names (e.g. `sqlite-outbox`, `bridge-opcua`).
    ///
    /// Calling `add_check` with an existing name REPLACES the
    /// previous closure — that's the "redefine on
    /// reconfiguration" path.
    pub async fn add_check<F, Fut>(&self, name: impl Into<String>, check: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = CheckResult> + Send + 'static,
    {
        let boxed: CheckFn = Arc::new(move || Box::pin(check()));
        self.checks.write().await.insert(name.into(), boxed);
    }

    /// Remove a registered check. Returns true if the name
    /// was registered, false otherwise.
    pub async fn remove_check(&self, name: &str) -> bool {
        self.checks.write().await.remove(name).is_some()
    }

    pub async fn check_count(&self) -> usize {
        self.checks.read().await.len()
    }

    /// Liveness — synchronous, no checks. The process being
    /// able to respond IS the liveness signal.
    pub fn live_body() -> HealthBody {
        HealthBody {
            status: "live",
            at: Utc::now(),
            failures: vec![],
        }
    }

    /// Readiness — runs every registered check (with the
    /// configured timeout each) and aggregates. Returns
    /// `(true, body)` for healthy / `(false, body)` for
    /// degraded so the caller can map to HTTP status.
    pub async fn ready(&self) -> (bool, HealthBody) {
        let snapshot: Vec<(String, CheckFn)> = self
            .checks
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut failures = Vec::new();
        for (name, check) in snapshot {
            let timed = tokio::time::timeout(self.check_timeout, check()).await;
            match timed {
                Ok(Ok(())) => {}
                Ok(Err(reason)) => failures.push(FailureDetail { name, reason }),
                Err(_) => failures.push(FailureDetail {
                    name,
                    reason: format!(
                        "check timed out after {} ms",
                        self.check_timeout.as_millis()
                    ),
                }),
            }
        }
        let healthy = failures.is_empty();
        (
            healthy,
            HealthBody {
                status: if healthy { "ready" } else { "degraded" },
                at: Utc::now(),
                failures,
            },
        )
    }
}

impl Default for HealthServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal in-process HTTP handler — no router framework.
/// Takes a path string and the server handle, returns the
/// (status, body) tuple ready for serialization.
pub async fn route(server: &HealthServer, path: &str) -> (u16, HealthBody) {
    match path {
        "/health" => (200, HealthServer::live_body()),
        "/ready" => {
            let (healthy, body) = server.ready().await;
            (if healthy { 200 } else { 503 }, body)
        }
        _ => (
            404,
            HealthBody {
                status: "not-found",
                at: Utc::now(),
                failures: vec![],
            },
        ),
    }
}

/// Render the response body to a JSON byte payload using
/// `serde_json`. Separated so the in-process tests don't
/// need to spin up a real HTTP server to verify the wire
/// shape.
pub fn body_to_json(body: &HealthBody) -> Vec<u8> {
    serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec())
}

/// Run a TCP listener that handles `/health` and `/ready`
/// using a minimal hand-rolled HTTP/1.0 responder. We
/// intentionally avoid hyper / axum / warp here so the
/// crate adds zero new dependencies — the protocol surface
/// (two routes, fixed JSON bodies, no streaming) is small
/// enough that a 50-line hand-roll is correct and auditable.
///
/// The future returned never completes under normal
/// operation; cancel by dropping it.
pub async fn serve(server: HealthServer, addr: SocketAddr) -> Result<(), HealthError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|source| HealthError::Bind { addr, source })?;

    loop {
        let (mut socket, _peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let server = server.clone();
        tokio::spawn(async move {
            // Cap request size — we only need the request
            // line. 1 KiB is generous; a hostile peer sending
            // 100 MiB shouldn't OOM us.
            let mut buf = vec![0u8; 1024];
            let Ok(n) = socket.read(&mut buf).await else {
                return;
            };
            let head = String::from_utf8_lossy(&buf[..n]);
            let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, body) = route(&server, &path).await;
            let body_bytes = body_to_json(&body);
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
                status = status,
                reason = match status {
                    200 => "OK",
                    503 => "Service Unavailable",
                    404 => "Not Found",
                    _ => "Unknown",
                },
                len = body_bytes.len(),
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.write_all(&body_bytes).await;
            let _ = socket.shutdown().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn live_body_status_is_live() {
        let body = HealthServer::live_body();
        assert_eq!(body.status, "live");
        assert!(body.failures.is_empty());
    }

    #[tokio::test]
    async fn fresh_server_with_no_checks_is_always_ready() {
        let server = HealthServer::new();
        assert_eq!(server.check_count().await, 0);
        let (healthy, body) = server.ready().await;
        assert!(healthy);
        assert_eq!(body.status, "ready");
        assert!(body.failures.is_empty());
    }

    #[tokio::test]
    async fn check_returning_ok_keeps_server_ready() {
        let server = HealthServer::new();
        server
            .add_check("always-healthy", || async { Ok(()) })
            .await;
        let (healthy, body) = server.ready().await;
        assert!(healthy);
        assert_eq!(body.status, "ready");
    }

    #[tokio::test]
    async fn failing_check_surfaces_name_and_reason() {
        let server = HealthServer::new();
        server
            .add_check("sqlite-outbox", || async { Err("locked".into()) })
            .await;
        let (healthy, body) = server.ready().await;
        assert!(!healthy);
        assert_eq!(body.status, "degraded");
        assert_eq!(body.failures.len(), 1);
        assert_eq!(body.failures[0].name, "sqlite-outbox");
        assert_eq!(body.failures[0].reason, "locked");
    }

    #[tokio::test]
    async fn timed_out_check_surfaces_as_failure_with_timeout_reason() {
        // Production checks should never block, but if one
        // does the server can't hang waiting — the timeout
        // surfaces it as a failure with a clear reason.
        let server = HealthServer::new().with_check_timeout(Duration::from_millis(50));
        server
            .add_check("slow-check", || async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok(())
            })
            .await;
        let (healthy, body) = server.ready().await;
        assert!(!healthy);
        assert_eq!(body.failures[0].name, "slow-check");
        assert!(body.failures[0].reason.contains("timed out"));
    }

    #[tokio::test]
    async fn multiple_failures_all_appear_in_body() {
        // Monitor's incident description shows every problem
        // at once; pin that no failure is silently dropped.
        let server = HealthServer::new();
        server
            .add_check("dep-a", || async { Err("nope".into()) })
            .await;
        server
            .add_check("dep-b", || async { Err("also nope".into()) })
            .await;
        let (healthy, body) = server.ready().await;
        assert!(!healthy);
        assert_eq!(body.failures.len(), 2);
        let names: Vec<&str> = body.failures.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"dep-a"));
        assert!(names.contains(&"dep-b"));
    }

    #[tokio::test]
    async fn add_check_replaces_existing_name() {
        // Reconfiguration path — the second `add_check` for
        // the same name takes over. Pin so a hot-config
        // refresh doesn't leave a stale closure.
        let server = HealthServer::new();
        server.add_check("dep", || async { Err("v1".into()) }).await;
        server.add_check("dep", || async { Ok(()) }).await;
        assert_eq!(server.check_count().await, 1);
        let (healthy, _) = server.ready().await;
        assert!(healthy);
    }

    #[tokio::test]
    async fn remove_check_removes_and_returns_true_when_present() {
        let server = HealthServer::new();
        server.add_check("dep", || async { Ok(()) }).await;
        assert!(server.remove_check("dep").await);
        assert_eq!(server.check_count().await, 0);
        assert!(!server.remove_check("dep").await);
    }

    #[tokio::test]
    async fn route_health_returns_200_live() {
        let server = HealthServer::new();
        let (status, body) = route(&server, "/health").await;
        assert_eq!(status, 200);
        assert_eq!(body.status, "live");
    }

    #[tokio::test]
    async fn route_ready_returns_503_when_any_check_fails() {
        let server = HealthServer::new();
        server.add_check("dep", || async { Err("x".into()) }).await;
        let (status, _) = route(&server, "/ready").await;
        assert_eq!(status, 503);
    }

    #[tokio::test]
    async fn route_unknown_path_returns_404() {
        let server = HealthServer::new();
        let (status, body) = route(&server, "/unknown").await;
        assert_eq!(status, 404);
        assert_eq!(body.status, "not-found");
    }

    #[tokio::test]
    async fn body_to_json_emits_expected_shape() {
        // Pin the wire format — monitor parsers depend on
        // the field names.
        let body = HealthBody {
            status: "ready",
            at: Utc::now(),
            failures: vec![],
        };
        let json = body_to_json(&body);
        let s = String::from_utf8_lossy(&json);
        assert!(s.contains("\"status\":\"ready\""));
        // `failures` is skipped when empty.
        assert!(!s.contains("\"failures\""));
    }

    #[tokio::test]
    async fn body_to_json_includes_failures_when_degraded() {
        let body = HealthBody {
            status: "degraded",
            at: Utc::now(),
            failures: vec![FailureDetail {
                name: "x".into(),
                reason: "y".into(),
            }],
        };
        let json = body_to_json(&body);
        let s = String::from_utf8_lossy(&json);
        assert!(s.contains("\"failures\""));
        assert!(s.contains("\"name\":\"x\""));
        assert!(s.contains("\"reason\":\"y\""));
    }

    #[tokio::test]
    async fn check_closure_is_invoked_each_ready_call() {
        // Pin that we don't cache check results — every
        // `/ready` request runs every check. Monitor expects
        // live data, not stale.
        let counter = Arc::new(AtomicU32::new(0));
        let server = HealthServer::new();
        {
            let counter = counter.clone();
            server
                .add_check("counted", move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })
                .await;
        }
        server.ready().await;
        server.ready().await;
        server.ready().await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
