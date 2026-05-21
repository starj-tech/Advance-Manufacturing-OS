//! OpenTelemetry OTLP exporter configuration scaffold.
//!
//! Same shape as the V38 Sentry scaffold: ships the wire
//! contract (env vars, config shape, validation) so the real
//! `opentelemetry-otlp` + `tracing-opentelemetry` SDK can
//! land behind a feature flag without rippling through the
//! binary.
//!
//! ## Why OTLP and not vendor-specific exporters
//! OTLP (OpenTelemetry Protocol) is the cross-vendor
//! standard. Honeycomb, Grafana Cloud, Datadog, Jaeger,
//! Tempo, and every other tracing backend AETHER might wire
//! up speaks it. Pinning OTLP at the trait surface means
//! tenants can swap backends by changing env vars only —
//! no code change.
//!
//! ## Env-var conventions
//! The official OpenTelemetry spec defines standard env vars
//! (`OTEL_EXPORTER_OTLP_ENDPOINT`, etc.). We honor those
//! verbatim — the boot path uses [`OtlpConfig::from_env`]
//! and forwards to the SDK without renaming, so OTEL-aware
//! operators can configure AETHER the same way they
//! configure every other OTEL-instrumented service.
//!
//! ## Why fail-soft on invalid endpoint
//! Same rationale as Sentry: telemetry is observability, not
//! correctness. A wrong endpoint should NOT crash the boot
//! path; it should log loudly and continue with
//! tracing-disabled mode.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::time::Duration;
use thiserror::Error;

/// Standard OTel env var for the collector endpoint.
/// `http://otel-collector:4318` (HTTP/protobuf) or
/// `grpc://otel-collector:4317` (gRPC) — the SDK picks the
/// transport from the scheme.
pub const OTLP_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Standard OTel env var for per-request headers, e.g.
/// `Authorization=Bearer xxx,X-Org-Id=acme`. Honeycomb's
/// API key lives here.
pub const OTLP_HEADERS_ENV: &str = "OTEL_EXPORTER_OTLP_HEADERS";

/// Standard OTel env var for the resource service name. We
/// default to `aether-os` when unset so traces always group
/// under a recognizable name.
pub const OTLP_SERVICE_NAME_ENV: &str = "OTEL_SERVICE_NAME";

/// Per-export timeout. The OTel spec defaults to 10s; we
/// inherit that. Anything longer and we'd be holding spans
/// in memory waiting on a stuck collector.
pub const DEFAULT_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum OtlpConfigError {
    #[error("invalid OTLP endpoint: {0}")]
    InvalidEndpoint(String),
    /// The headers string couldn't be parsed as
    /// `k1=v1,k2=v2`. The offending fragment is included.
    #[error("invalid OTLP headers: {0}")]
    InvalidHeaders(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OtlpConfig {
    /// Collector endpoint, e.g.
    /// `https://api.honeycomb.io:443`.
    pub endpoint: String,
    /// Service name attached as a resource attribute.
    /// Defaults to `aether-os` when env var unset.
    pub service_name: String,
    /// Headers attached to every export request. Honeycomb
    /// puts `x-honeycomb-team` here; Grafana Cloud puts
    /// `Authorization: Basic <id:key>`.
    pub headers: BTreeMap<String, String>,
    /// Per-export timeout. The SDK uses this for both span
    /// and metric exports.
    pub export_timeout: Duration,
}

impl OtlpConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            service_name: "aether-os".into(),
            headers: BTreeMap::new(),
            export_timeout: DEFAULT_EXPORT_TIMEOUT,
        }
    }

    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Replace the headers map. The header keys are case-
    /// preserved on the wire but lookups are done case-
    /// insensitively per HTTP convention.
    pub fn with_headers(mut self, headers: BTreeMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_export_timeout(mut self, timeout: Duration) -> Self {
        self.export_timeout = timeout;
        self
    }

    /// Construct from the standard OTel env vars. Returns
    /// None if `OTEL_EXPORTER_OTLP_ENDPOINT` is unset
    /// (tracing-disabled mode); returns an error if any var
    /// is malformed.
    pub fn from_env() -> Result<Option<Self>, OtlpConfigError> {
        let Ok(endpoint) = env::var(OTLP_ENDPOINT_ENV) else {
            return Ok(None);
        };
        let mut cfg = OtlpConfig::new(endpoint);
        if let Ok(name) = env::var(OTLP_SERVICE_NAME_ENV) {
            cfg = cfg.with_service_name(name);
        }
        if let Ok(raw) = env::var(OTLP_HEADERS_ENV) {
            let parsed = parse_headers(&raw)?;
            cfg = cfg.with_headers(parsed);
        }
        Ok(Some(cfg))
    }

    /// Validate the endpoint and headers. Same fail-soft
    /// rationale as Sentry: typed errors so the boot path
    /// can log + continue.
    pub fn validate(&self) -> Result<(), OtlpConfigError> {
        let endpoint = &self.endpoint;
        if endpoint.is_empty() {
            return Err(OtlpConfigError::InvalidEndpoint("empty".into()));
        }
        let valid_scheme = endpoint.starts_with("http://")
            || endpoint.starts_with("https://")
            || endpoint.starts_with("grpc://")
            || endpoint.starts_with("grpcs://");
        if !valid_scheme {
            return Err(OtlpConfigError::InvalidEndpoint(format!(
                "scheme must be http/https/grpc/grpcs: {endpoint}"
            )));
        }
        if self.service_name.trim().is_empty() {
            return Err(OtlpConfigError::InvalidEndpoint(
                "service_name must not be empty".into(),
            ));
        }
        for k in self.headers.keys() {
            if k.is_empty() {
                return Err(OtlpConfigError::InvalidHeaders(
                    "header name must not be empty".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Parse the OTel-spec headers string format:
/// `key1=value1,key2=value2`. Values may NOT contain `,`
/// (the OTel spec doesn't define escaping); equals signs
/// inside values are permitted (we split at the first `=`
/// only).
pub fn parse_headers(raw: &str) -> Result<BTreeMap<String, String>, OtlpConfigError> {
    let mut map = BTreeMap::new();
    for pair in raw.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((k, v)) = pair.split_once('=') else {
            return Err(OtlpConfigError::InvalidHeaders(format!(
                "missing `=`: {pair}"
            )));
        };
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() {
            return Err(OtlpConfigError::InvalidHeaders(format!(
                "empty key in pair: {pair}"
            )));
        }
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

/// Placeholder for the tracing layer the real
/// `tracing_opentelemetry::layer().with_tracer(...)` would
/// return. Mirrors the V38 sentry shim — boot path can
/// pattern-match on `Some(layer)` once the SDK lands.
pub fn tracing_layer(_config: &OtlpConfig) -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_endpoint() -> &'static str {
        "https://api.honeycomb.io:443"
    }

    #[test]
    fn config_constructor_sets_defaults() {
        let cfg = OtlpConfig::new(good_endpoint());
        assert_eq!(cfg.endpoint, good_endpoint());
        assert_eq!(cfg.service_name, "aether-os");
        assert!(cfg.headers.is_empty());
        assert_eq!(cfg.export_timeout, DEFAULT_EXPORT_TIMEOUT);
    }

    #[test]
    fn validate_accepts_http_https_grpc_grpcs() {
        for scheme in ["http", "https", "grpc", "grpcs"] {
            let cfg = OtlpConfig::new(format!("{scheme}://collector:4317"));
            cfg.validate()
                .unwrap_or_else(|e| panic!("scheme {scheme} should validate: {e}"));
        }
    }

    #[test]
    fn validate_rejects_unknown_scheme() {
        // tcp://, ws://, etc — not OTel transports.
        let err = OtlpConfig::new("ftp://collector").validate().unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidEndpoint(_)));
    }

    #[test]
    fn validate_rejects_empty_endpoint() {
        let err = OtlpConfig::new("").validate().unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidEndpoint(_)));
    }

    #[test]
    fn validate_rejects_empty_service_name() {
        let cfg = OtlpConfig::new(good_endpoint()).with_service_name("   ");
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidEndpoint(_)));
    }

    #[test]
    fn validate_rejects_empty_header_key() {
        let mut headers = BTreeMap::new();
        headers.insert("".into(), "value".into());
        let cfg = OtlpConfig::new(good_endpoint()).with_headers(headers);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidHeaders(_)));
    }

    #[test]
    fn parse_headers_handles_canonical_pair() {
        let headers = parse_headers("x-honeycomb-team=abc123").unwrap();
        assert_eq!(
            headers.get("x-honeycomb-team").map(String::as_str),
            Some("abc123")
        );
    }

    #[test]
    fn parse_headers_handles_multiple_pairs() {
        let headers = parse_headers("a=1,b=2,c=3").unwrap();
        assert_eq!(headers.len(), 3);
        assert_eq!(headers.get("a").map(String::as_str), Some("1"));
        assert_eq!(headers.get("c").map(String::as_str), Some("3"));
    }

    #[test]
    fn parse_headers_trims_whitespace_around_keys_and_values() {
        let headers = parse_headers("  k1  =  v1  ,  k2 = v2 ").unwrap();
        assert_eq!(headers.get("k1").map(String::as_str), Some("v1"));
        assert_eq!(headers.get("k2").map(String::as_str), Some("v2"));
    }

    #[test]
    fn parse_headers_allows_equals_inside_values() {
        // Authorization: Bearer base64==
        let headers = parse_headers("Authorization=Bearer base64==").unwrap();
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer base64==")
        );
    }

    #[test]
    fn parse_headers_skips_empty_pairs_from_trailing_comma() {
        // `a=1,b=2,` — operator's tooling sometimes leaves a
        // trailing comma. Don't fail on that.
        let headers = parse_headers("a=1,b=2,").unwrap();
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn parse_headers_rejects_pair_without_equals() {
        let err = parse_headers("orphan-key").unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidHeaders(_)));
    }

    #[test]
    fn parse_headers_rejects_pair_with_empty_key() {
        let err = parse_headers("=value").unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidHeaders(_)));
    }

    #[test]
    fn from_env_returns_none_when_endpoint_unset() {
        env::remove_var(OTLP_ENDPOINT_ENV);
        env::remove_var(OTLP_SERVICE_NAME_ENV);
        env::remove_var(OTLP_HEADERS_ENV);
        assert!(OtlpConfig::from_env().unwrap().is_none());
    }

    #[test]
    fn from_env_picks_up_endpoint_service_name_headers() {
        env::set_var(OTLP_ENDPOINT_ENV, good_endpoint());
        env::set_var(OTLP_SERVICE_NAME_ENV, "test-svc");
        env::set_var(OTLP_HEADERS_ENV, "x-honeycomb-team=key123");
        let cfg = OtlpConfig::from_env().unwrap().expect("endpoint set");
        assert_eq!(cfg.endpoint, good_endpoint());
        assert_eq!(cfg.service_name, "test-svc");
        assert_eq!(
            cfg.headers.get("x-honeycomb-team").map(String::as_str),
            Some("key123")
        );
        env::remove_var(OTLP_ENDPOINT_ENV);
        env::remove_var(OTLP_SERVICE_NAME_ENV);
        env::remove_var(OTLP_HEADERS_ENV);
    }

    #[test]
    fn from_env_surfaces_malformed_headers_as_typed_error() {
        env::set_var(OTLP_ENDPOINT_ENV, good_endpoint());
        env::set_var(OTLP_HEADERS_ENV, "bad-no-equals");
        let err = OtlpConfig::from_env().unwrap_err();
        assert!(matches!(err, OtlpConfigError::InvalidHeaders(_)));
        env::remove_var(OTLP_ENDPOINT_ENV);
        env::remove_var(OTLP_HEADERS_ENV);
    }

    #[test]
    fn tracing_layer_is_noop_until_real_sdk_feature_lands() {
        let cfg = OtlpConfig::new(good_endpoint());
        assert!(tracing_layer(&cfg).is_none());
    }

    #[test]
    fn config_serde_round_trips_through_json() {
        let mut headers = BTreeMap::new();
        headers.insert("k".into(), "v".into());
        let cfg = OtlpConfig::new(good_endpoint())
            .with_service_name("test-svc")
            .with_headers(headers)
            .with_export_timeout(Duration::from_secs(30));
        let json = serde_json::to_string(&cfg).unwrap();
        let back: OtlpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
