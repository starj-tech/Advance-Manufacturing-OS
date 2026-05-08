use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SosTier {
    /// Hardware button or in-app long-press.
    Hardware,
    /// mDNS UDP multicast on the LAN.
    LocalLan,
    /// MQTT broker (factory backbone).
    Mqtt,
    /// Supabase Realtime row insert.
    Cloud,
    /// SMS/Slack/email/webhook from Edge Function.
    External,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SosEvent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub triggered_at: DateTime<Utc>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub accuracy_m: Option<u32>,
    pub note: Option<String>,
}

#[derive(Debug, Error)]
pub enum SosError {
    #[error("network: {0}")]
    Network(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Multi-tier SOS pipeline. Implementations broadcast through every
/// available tier in parallel; the cloud tier is authoritative for
/// audit but local tiers must succeed even when WAN is down.
#[async_trait::async_trait]
pub trait SosBroadcaster: Send + Sync {
    async fn broadcast(&self, event: &SosEvent) -> Result<Vec<SosTier>, SosError>;
}

/// No-op skeleton that reports zero tiers reached. Wired up in PR #5.
pub struct NoopBroadcaster;

#[async_trait::async_trait]
impl SosBroadcaster for NoopBroadcaster {
    async fn broadcast(&self, _event: &SosEvent) -> Result<Vec<SosTier>, SosError> {
        Err(SosError::NotImplemented("SosBroadcaster wired in PR #5"))
    }
}
