//! Multi-tier SOS broadcast pipeline.
//!
//! ## Why multiple tiers in parallel
//! An emergency SOS needs to reach SOMEONE even when most of
//! the network is broken. The PR #5 plan calls for five tiers:
//!
//!   * Hardware — physical button / hold-press triggers the
//!     local event loop. This tier "succeeds" inherently when
//!     the operator pressed the button (the event exists).
//!   * LocalLan — mDNS UDP multicast. Works when WAN is down
//!     and the in-factory router segregates the LAN.
//!   * Mqtt — factory backbone broker. Different failure mode
//!     from LAN multicast (broker can be up when multicast is
//!     blocked, and vice versa).
//!   * Cloud — Supabase Realtime row insert. Authoritative for
//!     audit; only tier that survives a complete factory
//!     outage (off-site responders see it).
//!   * External — SMS / Slack / email / webhook from an Edge
//!     Function. Reaches humans who aren't watching the app.
//!
//! [`MultiTierBroadcaster`] fans out the event to every
//! configured tier in parallel via `tokio::join` and reports
//! the set of tiers that actually succeeded. Real per-tier
//! impls (mDNS via mdns-sd, MQTT via rumqttc, Realtime via
//! reqwest, External via an Edge Function) plug in later; the
//! orchestration is what we pin here.
//!
//! ## Error semantics
//! - `Ok(vec![tiers_succeeded])` when at least one tier got
//!   through. The vec is sorted in canonical [`SosTier`]
//!   declaration order so a caller can write `if
//!   succeeded.contains(&SosTier::Cloud) {...}` deterministically.
//! - `Err(AllTiersFailed)` when EVERY configured tier
//!   returned an error. This is the "factory is on fire AND
//!   the LAN is down AND the cellular fallback is dead" case
//!   — extremely rare, but the typed error lets the UI
//!   surface "could not raise alarm; physically intervene"
//!   instead of pretending success.
//!
//! ## What this layer does NOT do
//! - Dedupe. An operator pressing SOS twice on a stuck event
//!   is signalling urgency; broadcast both. Cross-event
//!   dedupe by `event.id` is the recipient's concern.
//! - Rate-limit. Same rationale. Telemetry pipelines (V23)
//!   batch, SOS does not.
//! - Geofence enrichment. The event arrives with location
//!   metadata already populated by the caller (`aether-
//!   safety::geofence`).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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

impl SosTier {
    /// Kebab-case slug for the audit ledger.
    pub fn slug(&self) -> &'static str {
        match self {
            SosTier::Hardware => "hardware",
            SosTier::LocalLan => "local-lan",
            SosTier::Mqtt => "mqtt",
            SosTier::Cloud => "cloud",
            SosTier::External => "external",
        }
    }
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
    /// Every configured tier returned an error. Rare; signals
    /// the operator should physically intervene because
    /// software couldn't reach anyone.
    #[error("every configured tier failed; {0} attempts")]
    AllTiersFailed(usize),
    /// Builder was used with zero tiers. Programmer error —
    /// a no-tier broadcaster would always claim failure with
    /// no signal of what was wrong.
    #[error("MultiTierBroadcaster requires at least one tier")]
    NoTiersConfigured,
}

/// One-tier broadcaster surface. Implementations broadcast
/// over their specific transport and return `Ok(())` on
/// successful publish. The aggregator wraps each impl with
/// its `SosTier` tag.
#[async_trait]
pub trait SosBroadcaster: Send + Sync {
    async fn broadcast(&self, event: &SosEvent) -> Result<Vec<SosTier>, SosError>;
}

/// Composite broadcaster that fans an event out to N inner
/// broadcasters in parallel. The unit of composition: a
/// `(SosTier, Arc<dyn SosBroadcaster>)` pair.
pub struct MultiTierBroadcaster {
    tiers: Vec<(SosTier, Arc<dyn SosBroadcaster>)>,
}

impl MultiTierBroadcaster {
    pub fn new() -> Self {
        Self { tiers: Vec::new() }
    }

    /// Add a tier impl. Same tier may be added twice (two
    /// LAN multicast addresses, say); both run in parallel
    /// and either success satisfies that tier.
    pub fn with_tier(mut self, tier: SosTier, broadcaster: Arc<dyn SosBroadcaster>) -> Self {
        self.tiers.push((tier, broadcaster));
        self
    }

    pub fn tier_count(&self) -> usize {
        self.tiers.len()
    }
}

impl Default for MultiTierBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SosBroadcaster for MultiTierBroadcaster {
    async fn broadcast(&self, event: &SosEvent) -> Result<Vec<SosTier>, SosError> {
        if self.tiers.is_empty() {
            return Err(SosError::NoTiersConfigured);
        }
        // Fan out in parallel. Each inner future returns
        // (tier_tag, Result<_>) so we can attribute
        // successes / failures back to their tier.
        let futures = self.tiers.iter().map(|(tier, broadcaster)| {
            let tier = *tier;
            let broadcaster = broadcaster.clone();
            async move {
                let result = broadcaster.broadcast(event).await;
                (tier, result)
            }
        });
        let results: Vec<(SosTier, Result<Vec<SosTier>, SosError>)> =
            futures::future::join_all(futures).await;

        let attempts = results.len();
        let mut succeeded: Vec<SosTier> = results
            .into_iter()
            .filter_map(|(t, r)| r.ok().map(|_| t))
            .collect();
        if succeeded.is_empty() {
            return Err(SosError::AllTiersFailed(attempts));
        }
        // Canonical ordering — caller can predicate-check
        // `succeeded.contains(&Cloud)` without worrying about
        // race-determined order.
        succeeded.sort();
        succeeded.dedup();
        Ok(succeeded)
    }
}

/// Test fixture: a one-tier broadcaster that succeeds or
/// fails deterministically based on construction param.
/// Production broadcasters (mDNS / MQTT / Cloud / External)
/// plug into the same trait surface in later sessions.
pub struct MockTier {
    succeed: bool,
}

impl MockTier {
    pub fn succeeding() -> Self {
        Self { succeed: true }
    }
    pub fn failing() -> Self {
        Self { succeed: false }
    }
}

#[async_trait]
impl SosBroadcaster for MockTier {
    async fn broadcast(&self, _event: &SosEvent) -> Result<Vec<SosTier>, SosError> {
        if self.succeed {
            Ok(vec![])
        } else {
            Err(SosError::Network("simulated".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_event() -> SosEvent {
        SosEvent {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            triggered_at: Utc::now(),
            lat: Some(-6.21),
            lng: Some(106.82),
            accuracy_m: Some(8),
            note: Some("fall detected".into()),
        }
    }

    #[tokio::test]
    async fn empty_broadcaster_returns_no_tiers_configured() {
        let b = MultiTierBroadcaster::new();
        let err = b.broadcast(&fixture_event()).await.unwrap_err();
        assert!(matches!(err, SosError::NoTiersConfigured));
    }

    #[tokio::test]
    async fn single_succeeding_tier_returns_that_tier() {
        let b =
            MultiTierBroadcaster::new().with_tier(SosTier::Cloud, Arc::new(MockTier::succeeding()));
        let tiers = b.broadcast(&fixture_event()).await.unwrap();
        assert_eq!(tiers, vec![SosTier::Cloud]);
    }

    #[tokio::test]
    async fn every_tier_failing_surfaces_all_tiers_failed() {
        let b = MultiTierBroadcaster::new()
            .with_tier(SosTier::Cloud, Arc::new(MockTier::failing()))
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::failing()))
            .with_tier(SosTier::External, Arc::new(MockTier::failing()));
        let err = b.broadcast(&fixture_event()).await.unwrap_err();
        match err {
            SosError::AllTiersFailed(n) => assert_eq!(n, 3),
            other => panic!("expected AllTiersFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn partial_success_reports_only_succeeded_tiers() {
        // Cloud succeeds, others fail — the typical degraded
        // mode (WAN up but factory LAN down).
        let b = MultiTierBroadcaster::new()
            .with_tier(SosTier::LocalLan, Arc::new(MockTier::failing()))
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::failing()))
            .with_tier(SosTier::Cloud, Arc::new(MockTier::succeeding()))
            .with_tier(SosTier::External, Arc::new(MockTier::succeeding()));
        let tiers = b.broadcast(&fixture_event()).await.unwrap();
        // Sorted in SosTier declaration order: Cloud(3) <
        // External(4). LocalLan(1) + Mqtt(2) failed so they
        // don't appear.
        assert_eq!(tiers, vec![SosTier::Cloud, SosTier::External]);
    }

    #[tokio::test]
    async fn duplicate_tier_entries_dedupe_in_output() {
        // A real deployment might wire two MQTT brokers
        // (primary + failover). Both running counts as one
        // tier in the output set.
        let b = MultiTierBroadcaster::new()
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::succeeding()))
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::succeeding()));
        let tiers = b.broadcast(&fixture_event()).await.unwrap();
        assert_eq!(tiers, vec![SosTier::Mqtt]);
    }

    #[tokio::test]
    async fn at_least_one_succeeding_among_failures_returns_ok() {
        // Headline reliability test: 4 of 5 tiers fail, 1
        // tier succeeds → caller still sees Ok with the
        // surviving tier. Pins the "factory operator survives
        // any single-network outage" property.
        let b = MultiTierBroadcaster::new()
            .with_tier(SosTier::Hardware, Arc::new(MockTier::failing()))
            .with_tier(SosTier::LocalLan, Arc::new(MockTier::failing()))
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::failing()))
            .with_tier(SosTier::Cloud, Arc::new(MockTier::succeeding()))
            .with_tier(SosTier::External, Arc::new(MockTier::failing()));
        let tiers = b.broadcast(&fixture_event()).await.unwrap();
        assert_eq!(tiers, vec![SosTier::Cloud]);
    }

    #[tokio::test]
    async fn tier_count_reports_configured_size() {
        let b = MultiTierBroadcaster::new()
            .with_tier(SosTier::Cloud, Arc::new(MockTier::succeeding()))
            .with_tier(SosTier::Mqtt, Arc::new(MockTier::succeeding()));
        assert_eq!(b.tier_count(), 2);
    }

    #[test]
    fn tier_slugs_distinct_and_kebab_cased() {
        let kinds = [
            SosTier::Hardware,
            SosTier::LocalLan,
            SosTier::Mqtt,
            SosTier::Cloud,
            SosTier::External,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|t| t.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }
}
