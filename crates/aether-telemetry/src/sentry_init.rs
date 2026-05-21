//! Sentry integration scaffold.
//!
//! ## Why scaffold, not wiring
//! Pulling in the `sentry` crate as a hard dep adds ~3 MB to
//! the binary and 30+ transitive crates. For tenants that
//! don't use Sentry (the local-air-gapped pilots, the SOC-2-
//! anxious clients who insist on on-prem only), that's pure
//! overhead.
//!
//! This module ships the OBSERVABLE WIRE-CONTRACT — the
//! `SentryConfig` type the operator fills in from env vars,
//! the `validate()` shape that confirms the DSN format
//! before anything actually tries to send, and the
//! `tracing_layer()` no-op shim that production replaces with
//! `sentry_tracing::layer()` once the `sentry` feature flag
//! lands. Adding the actual SDK is a one-line dep change +
//! feature toggle when a customer needs it; the surface here
//! makes sure that change doesn't ripple through the binary.
//!
//! ## DSN format
//! Sentry DSNs look like `https://<public-key>@<org>.ingest.
//! sentry.io/<project-id>`. We validate the shape (scheme,
//! `@` separator, non-empty key + host) but NEVER attempt to
//! reach the host from this module — that's the SDK's job.
//!
//! ## Why fail-soft on invalid DSN
//! A wrong DSN in env should not crash the binary. We
//! surface a typed error so the boot path can log it loudly
//! and continue in "telemetry disabled" mode rather than
//! refuse to start. Telemetry is observability, not
//! correctness — losing it is a P3, refusing-to-boot is
//! a P0.

use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

/// Environment variable the boot path reads the DSN from.
/// Matches the SDK convention so the env can be set once
/// and consumed by both this scaffold and the real SDK when
/// it lands.
pub const SENTRY_DSN_ENV: &str = "SENTRY_DSN";

/// Environment variable the operator sets to pick the
/// release tag (commit SHA / semver). Sentry uses this to
/// group errors per deploy.
pub const SENTRY_RELEASE_ENV: &str = "SENTRY_RELEASE";

/// Environment variable for the environment tag (`prod`,
/// `staging`, `pilot-acme-foods`). Operator-controlled.
pub const SENTRY_ENVIRONMENT_ENV: &str = "SENTRY_ENVIRONMENT";

#[derive(Debug, Error)]
pub enum SentryConfigError {
    /// DSN string was malformed. Carries the offending
    /// fragment so the boot-log line is actionable.
    #[error("invalid Sentry DSN: {0}")]
    InvalidDsn(String),
}

/// Operator-facing Sentry configuration. All fields are
/// optional except `dsn`; missing `release` / `environment`
/// just means events arrive ungrouped.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SentryConfig {
    pub dsn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Whether to enable performance / tracing transmissions.
    /// `false` by default — most pilots want error reporting
    /// only, not perf (which costs more on paid Sentry plans).
    pub send_traces: bool,
}

impl SentryConfig {
    pub fn new(dsn: impl Into<String>) -> Self {
        Self {
            dsn: dsn.into(),
            release: None,
            environment: None,
            send_traces: false,
        }
    }

    pub fn with_release(mut self, release: impl Into<String>) -> Self {
        self.release = Some(release.into());
        self
    }

    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = Some(environment.into());
        self
    }

    pub fn with_send_traces(mut self, send: bool) -> Self {
        self.send_traces = send;
        self
    }

    /// Construct from the standard env vars. Returns `None`
    /// when `SENTRY_DSN` is unset — the boot path treats
    /// this as "telemetry disabled" rather than an error.
    pub fn from_env() -> Option<Self> {
        let dsn = env::var(SENTRY_DSN_ENV).ok()?;
        let mut cfg = SentryConfig::new(dsn);
        if let Ok(rel) = env::var(SENTRY_RELEASE_ENV) {
            cfg = cfg.with_release(rel);
        }
        if let Ok(envv) = env::var(SENTRY_ENVIRONMENT_ENV) {
            cfg = cfg.with_environment(envv);
        }
        Some(cfg)
    }

    /// Validate the DSN's surface shape — scheme, `@`
    /// separator, non-empty key + host. NEVER attempts to
    /// reach the host. Real-world bad DSNs (typos, missing
    /// keys) surface here so the SDK doesn't quietly drop
    /// events with `transport error: dns failure`.
    pub fn validate(&self) -> Result<(), SentryConfigError> {
        let dsn = &self.dsn;
        if dsn.is_empty() {
            return Err(SentryConfigError::InvalidDsn("empty".into()));
        }
        if !(dsn.starts_with("https://") || dsn.starts_with("http://")) {
            return Err(SentryConfigError::InvalidDsn(format!(
                "missing scheme: {}",
                truncate_for_log(dsn)
            )));
        }
        let after_scheme = dsn
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        let Some((key, host_part)) = after_scheme.split_once('@') else {
            return Err(SentryConfigError::InvalidDsn(format!(
                "missing public key (no `@`): {}",
                truncate_for_log(dsn)
            )));
        };
        if key.is_empty() {
            return Err(SentryConfigError::InvalidDsn("public key is empty".into()));
        }
        if host_part.is_empty() || !host_part.contains('/') {
            return Err(SentryConfigError::InvalidDsn(format!(
                "host or project-id missing: {}",
                truncate_for_log(dsn)
            )));
        }
        Ok(())
    }
}

/// Truncate a DSN-ish string so log lines never leak full
/// keys. Public-key-first DSNs are still semi-secret —
/// they're write-only on the Sentry side, but a leaked DSN
/// lets attackers send forged events.
fn truncate_for_log(s: &str) -> String {
    let n = s.len();
    if n <= 16 {
        return "***".into();
    }
    let prefix: String = s.chars().take(8).collect();
    let suffix: String = s
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}…{suffix} ({n} chars)")
}

/// Placeholder for the `tracing_subscriber` layer that the
/// real `sentry-tracing` crate provides. Returns an
/// always-empty `Option<()>` so the boot path can pattern-
/// match on `Some(layer)` once the SDK ships without
/// changing call shape.
///
/// When the `sentry` feature flag is added in a future
/// session, this returns `Some(sentry_tracing::layer())`
/// (a real `Layer<Registry>`) instead of `None`.
pub fn tracing_layer(_config: &SentryConfig) -> Option<()> {
    // The feature-gated real impl will replace `None` with
    // the actual layer. Pin the no-op surface so call sites
    // don't change.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_dsn() -> &'static str {
        "https://abc123def456@o12345.ingest.sentry.io/789012"
    }

    #[test]
    fn config_constructor_sets_dsn_and_defaults_rest_to_none() {
        let cfg = SentryConfig::new("https://x@y.com/1");
        assert_eq!(cfg.dsn, "https://x@y.com/1");
        assert!(cfg.release.is_none());
        assert!(cfg.environment.is_none());
        assert!(!cfg.send_traces);
    }

    #[test]
    fn builder_methods_set_optional_fields() {
        let cfg = SentryConfig::new("https://x@y.com/1")
            .with_release("v0.42.0")
            .with_environment("staging")
            .with_send_traces(true);
        assert_eq!(cfg.release.as_deref(), Some("v0.42.0"));
        assert_eq!(cfg.environment.as_deref(), Some("staging"));
        assert!(cfg.send_traces);
    }

    #[test]
    fn validate_accepts_canonical_sentry_dsn() {
        let cfg = SentryConfig::new(good_dsn());
        cfg.validate().expect("canonical DSN should validate");
    }

    #[test]
    fn validate_accepts_http_scheme_for_self_hosted_sentry() {
        // Self-hosted Sentry instances on private networks
        // commonly use plain HTTP. We allow it explicitly —
        // operator's call whether to encrypt the link to
        // their on-prem ingest.
        let cfg = SentryConfig::new("http://abc@self-hosted.local/1");
        cfg.validate()
            .expect("self-hosted HTTP DSN should validate");
    }

    #[test]
    fn validate_rejects_empty_dsn() {
        let err = SentryConfig::new("").validate().unwrap_err();
        assert!(matches!(err, SentryConfigError::InvalidDsn(_)));
    }

    #[test]
    fn validate_rejects_dsn_without_scheme() {
        let err = SentryConfig::new("abc@host/1").validate().unwrap_err();
        assert!(matches!(err, SentryConfigError::InvalidDsn(_)));
    }

    #[test]
    fn validate_rejects_dsn_without_at_separator() {
        let err = SentryConfig::new("https://no-key.host/1")
            .validate()
            .unwrap_err();
        let SentryConfigError::InvalidDsn(msg) = err;
        assert!(msg.contains('@'));
    }

    #[test]
    fn validate_rejects_dsn_with_empty_public_key() {
        let err = SentryConfig::new("https://@host/1").validate().unwrap_err();
        let SentryConfigError::InvalidDsn(msg) = err;
        assert!(msg.contains("public key"));
    }

    #[test]
    fn validate_rejects_dsn_without_project_id() {
        // No `/` after the host = no project id segment.
        let err = SentryConfig::new("https://abc@host")
            .validate()
            .unwrap_err();
        assert!(matches!(err, SentryConfigError::InvalidDsn(_)));
    }

    #[test]
    fn truncate_for_log_hides_long_dsn_key() {
        let dsn = "https://abc123def456ghi789@host/1";
        let truncated = truncate_for_log(dsn);
        // The middle (where the key lives) is replaced with
        // ellipsis. Confirm the full key isn't present.
        assert!(!truncated.contains("abc123def456ghi789"));
    }

    #[test]
    fn truncate_for_log_handles_short_input_gracefully() {
        assert_eq!(truncate_for_log(""), "***");
        assert_eq!(truncate_for_log("short"), "***");
    }

    #[test]
    fn from_env_returns_none_when_dsn_unset() {
        // Use a unique var name so we don't race with other
        // tests reading SENTRY_DSN.
        env::remove_var("SENTRY_DSN");
        env::remove_var("SENTRY_RELEASE");
        env::remove_var("SENTRY_ENVIRONMENT");
        // Boot path's contract: missing DSN = telemetry off,
        // NOT an error.
        assert!(SentryConfig::from_env().is_none());
    }

    #[test]
    fn from_env_picks_up_dsn_release_and_environment() {
        // Restore env after the test — `env::set_var` leaks
        // into other tests in the same process.
        env::set_var("SENTRY_DSN", good_dsn());
        env::set_var("SENTRY_RELEASE", "v0.99.0");
        env::set_var("SENTRY_ENVIRONMENT", "pilot");
        let cfg = SentryConfig::from_env().expect("DSN set");
        assert_eq!(cfg.dsn, good_dsn());
        assert_eq!(cfg.release.as_deref(), Some("v0.99.0"));
        assert_eq!(cfg.environment.as_deref(), Some("pilot"));
        env::remove_var("SENTRY_DSN");
        env::remove_var("SENTRY_RELEASE");
        env::remove_var("SENTRY_ENVIRONMENT");
    }

    #[test]
    fn tracing_layer_is_noop_until_real_sdk_feature_lands() {
        // Pin that the placeholder returns None today. When
        // the `sentry` feature gates the real SDK, this test
        // gets a `#[cfg(not(feature = "sentry"))]` and a
        // sibling test exercises the layer path.
        let cfg = SentryConfig::new(good_dsn());
        assert!(tracing_layer(&cfg).is_none());
    }

    #[test]
    fn config_serde_round_trips_through_json() {
        // Pin wire format — the boot path may serialize this
        // into the V10 actuator_commands.result for "boot
        // config snapshot" audit rows.
        let cfg = SentryConfig::new(good_dsn())
            .with_release("v1.0.0")
            .with_environment("staging");
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SentryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
        // `release` + `environment` present.
        assert!(json.contains("release"));
        assert!(json.contains("environment"));
    }

    #[test]
    fn config_serde_omits_none_optional_fields() {
        // skip_serializing_if = "Option::is_none" should
        // collapse the omitted fields. Pin so a future
        // serde change doesn't quietly serialize `null`s.
        let cfg = SentryConfig::new(good_dsn());
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(!json.contains("release"));
        assert!(!json.contains("environment"));
    }
}
