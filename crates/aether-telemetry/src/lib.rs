//! Tracing + metrics initialization, plus a backpressure-aware
//! batch aggregator for telemetry pipelines that need to throttle
//! producers against a slow durable sink.

pub mod batch;
pub mod circuit;
pub mod circuit_symptom;
pub mod health;
pub mod otlp;
pub mod sentry_init;

pub use batch::{
    spawn_batcher, BatchConfig, BatchHandle, DEFAULT_BATCH_MAX_ITEMS, DEFAULT_CHANNEL_CAPACITY,
    DEFAULT_FLUSH_INTERVAL,
};
pub use circuit::{
    BreakerConfig, BreakerState, CircuitBreaker, Decision, DEFAULT_COOLDOWN,
    DEFAULT_FAILURE_THRESHOLD,
};
pub use circuit_symptom::{
    breaker_symptom, breaker_symptoms, BreakerSymptom, KIND_BREAKER_HALF_OPEN, KIND_BREAKER_OPEN,
};
pub use health::{
    body_to_json, route as health_route, serve as serve_health, CheckResult, FailureDetail,
    HealthBody, HealthError, HealthServer, DEFAULT_CHECK_TIMEOUT, DEFAULT_HEALTH_PORT,
};
pub use otlp::{
    parse_headers as parse_otlp_headers, tracing_layer as otlp_tracing_layer, OtlpConfig,
    OtlpConfigError, DEFAULT_EXPORT_TIMEOUT, OTLP_ENDPOINT_ENV, OTLP_HEADERS_ENV,
    OTLP_SERVICE_NAME_ENV,
};
pub use sentry_init::{
    tracing_layer as sentry_tracing_layer, SentryConfig, SentryConfigError, SENTRY_DSN_ENV,
    SENTRY_ENVIRONMENT_ENV, SENTRY_RELEASE_ENV,
};

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize the global tracing subscriber.
///
/// Reads filter directives from `AETHER_LOG` (preferred) or `RUST_LOG`,
/// defaulting to `info` for the AETHER crates and `warn` for the rest.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_env("AETHER_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| {
            EnvFilter::new("warn,aether_core=info,aether_db=info,aether_sync=info,aether_protocols=info,aether_modules=info,aether_safety=info,aether_desktop_lib=info")
        });

    let layer = fmt::layer().with_target(true).with_level(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init()
        .ok();
}
