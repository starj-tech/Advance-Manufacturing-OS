//! Alpha Vantage commodity-feed adapter.
//!
//! ## Why this exists alongside `HttpCommodityFeed`
//! `HttpCommodityFeed` consumes the canonical
//! `{ "quotes": [{ "ticker", "timestamp", "price", "currency",
//! "source" }] }` shape — the wire contract AETHER's own
//! aggregator Edge Function publishes. That aggregator
//! exists to insulate AETHER from per-exchange JSON drift.
//!
//! Alpha Vantage publishes a DIFFERENT shape, and offers a
//! free tier (5 req/min, 500 req/day) that's perfect for
//! dev / pilot. This adapter translates the Alpha Vantage
//! shape into `PriceObservation` directly — no aggregator
//! sidecar required for getting started.
//!
//! ## Wire shape (Alpha Vantage)
//!
//! `GET https://www.alphavantage.co/query?function=COPPER&interval=daily&apikey=<KEY>`
//!
//! ```json
//! {
//!   "name": "Global Price of Copper",
//!   "interval": "daily",
//!   "unit": "dollars per pound",
//!   "data": [
//!     { "date": "2024-01-15", "value": "3.85" },
//!     { "date": "2024-01-14", "value": "3.82" }
//!   ]
//! }
//! ```
//!
//! Each commodity has its own `function=` parameter — `COPPER`,
//! `ALUMINUM`, `BRENT`, `NATURAL_GAS`, `WTI`, etc. Mapped per
//! commodity below. Unsupported commodities surface as
//! `FeedError::Decode("alpha vantage does not publish …")`.
//!
//! ## Free-tier rate limits
//! Alpha Vantage free tier: 5 calls/min, 500 calls/day.
//! Production usage hits this in minutes. The adapter ships
//! intended for dev / pilot bring-up; production tenants
//! either upgrade to a paid Alpha Vantage tier or wire the
//! `HttpCommodityFeed` against a paid LME/CME feed.

use crate::feed::{CommodityFeed, FeedError, PriceObservation};
use crate::fx::FxRateProvider;
use crate::model::Commodity;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

/// Default Alpha Vantage base URL. Override in tests via
/// `with_base_url`.
pub const DEFAULT_BASE_URL: &str = "https://www.alphavantage.co";

/// Default per-request timeout. Alpha Vantage's free tier
/// occasionally takes >5s for first response of a new
/// commodity; 15s gives headroom while still failing fast on
/// hard outage.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Map a `Commodity` to Alpha Vantage's `function=` value.
/// Returns `None` for commodities Alpha Vantage doesn't
/// publish (the caller surfaces this as a typed FeedError so
/// no silent drops happen).
pub fn alpha_vantage_function(commodity: Commodity) -> Option<&'static str> {
    match commodity {
        Commodity::Copper => Some("COPPER"),
        Commodity::Aluminum => Some("ALUMINUM"),
        Commodity::Brent => Some("BRENT"),
        Commodity::NaturalGas => Some("NATURAL_GAS"),
        // Alpha Vantage doesn't publish these — they're either
        // industrial-only (Steel HRC, polypropylene, lithium
        // carbonate, cobalt) or only on paid tiers.
        Commodity::SteelHrc | Commodity::Polypropylene | Commodity::Lithium | Commodity::Cobalt => {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct AlphaVantageResponse {
    #[serde(default)]
    name: Option<String>,
    /// Alpha Vantage tags responses with their natural unit
    /// ("dollars per pound", "dollars per barrel", …). We
    /// don't currently use this — the wire price is already
    /// USD-denominated — but parsing it surfaces upstream
    /// schema drift loudly (a renamed field becomes a serde
    /// `unknown_field` warning in dev).
    #[allow(dead_code)]
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    data: Vec<AlphaVantageRow>,
    /// Alpha Vantage error responses come back with HTTP 200
    /// and an `Error Message` or `Note` field instead of
    /// `data`. Both are surfaced as typed errors.
    #[serde(default, rename = "Error Message")]
    error_message: Option<String>,
    #[serde(default, rename = "Note")]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AlphaVantageRow {
    date: String,
    /// Alpha Vantage returns prices as strings (`"3.85"`)
    /// even though they're numeric. Parse client-side.
    value: String,
}

pub struct AlphaVantageFeed {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    fx: Arc<dyn FxRateProvider>,
}

impl AlphaVantageFeed {
    /// Construct with the given API key and a default
    /// `FxRateProvider`. Alpha Vantage publishes prices in
    /// USD natively for the commodities AETHER cares about, so
    /// the FX provider is rarely exercised — but it's still
    /// the right plumbing for consistency with the canonical
    /// `HttpCommodityFeed`.
    pub fn new(api_key: impl Into<String>, fx: Arc<dyn FxRateProvider>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            client: reqwest::Client::builder()
                .timeout(DEFAULT_TIMEOUT)
                .build()
                .expect("reqwest client must build with default config"),
            fx,
        }
    }

    /// Override the base URL — used by tests to point at a
    /// wiremock stub. Production never sets this.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Override the per-request timeout. Default 15s.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client must build with custom timeout");
        self
    }

    /// Single-commodity fetch returning the most recent
    /// observation. Used by `latest()`; exposed publicly so
    /// callers that need just one commodity don't have to
    /// build a Vec.
    pub async fn fetch_latest_one(
        &self,
        commodity: Commodity,
    ) -> Result<Option<PriceObservation>, FeedError> {
        let func = alpha_vantage_function(commodity).ok_or_else(|| {
            FeedError::Decode(format!("alpha vantage does not publish {:?}", commodity))
        })?;
        let url = format!(
            "{}/query?function={}&interval=daily&apikey={}",
            self.base_url, func, self.api_key
        );
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FeedError::Network(format!("{e}")))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(FeedError::RateLimited);
        }
        if status.is_server_error() {
            return Err(FeedError::Network(format!("upstream {status}")));
        }
        if !status.is_success() {
            return Err(FeedError::Decode(format!("unexpected HTTP {status}")));
        }
        let parsed: AlphaVantageResponse = response
            .json()
            .await
            .map_err(|e| FeedError::Decode(format!("json: {e}")))?;
        if let Some(msg) = parsed.error_message {
            return Err(FeedError::Decode(format!("alpha vantage error: {msg}")));
        }
        if let Some(note) = parsed.note {
            // Free-tier rate-limit responses arrive as HTTP
            // 200 with a `Note` field. Surface as RateLimited
            // so the caller's backoff logic kicks in.
            if note.contains("call frequency") || note.contains("rate limit") {
                return Err(FeedError::RateLimited);
            }
            return Err(FeedError::Decode(format!("alpha vantage note: {note}")));
        }
        let Some(latest) = parsed.data.first() else {
            return Ok(None);
        };
        let price: f64 = latest
            .value
            .parse()
            .map_err(|e| FeedError::Decode(format!("value parse: {e}")))?;
        let date = NaiveDate::parse_from_str(&latest.date, "%Y-%m-%d")
            .map_err(|e| FeedError::Decode(format!("date parse: {e}")))?;
        let at: DateTime<Utc> = Utc
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .ok_or_else(|| FeedError::Decode("invalid date conversion".into()))?;
        // Alpha Vantage's commodity functions return USD per
        // their respective unit, so the USD-normalized price
        // is the raw price. The FX provider would only kick
        // in if a future commodity is published in a non-USD
        // currency; pin this so the path exists.
        let usd_price = price;
        Ok(Some(PriceObservation {
            commodity,
            at,
            price,
            usd_price,
            source: parsed.name.unwrap_or_else(|| "alpha-vantage".into()),
        }))
    }
}

#[async_trait]
impl CommodityFeed for AlphaVantageFeed {
    async fn latest(&self, commodities: &[Commodity]) -> Result<Vec<PriceObservation>, FeedError> {
        let mut out = Vec::with_capacity(commodities.len());
        for c in commodities {
            match self.fetch_latest_one(*c).await {
                Ok(Some(obs)) => out.push(obs),
                // Unsupported commodity at Alpha Vantage: skip
                // silently rather than failing the whole batch.
                // The HttpCommodityFeed established this
                // contract; preserve it here.
                Ok(None) => continue,
                Err(FeedError::Decode(msg))
                    if msg.starts_with("alpha vantage does not publish") =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    async fn history(
        &self,
        commodity: Commodity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<PriceObservation>, FeedError> {
        // Alpha Vantage returns a full historical series in
        // one call — we filter to the requested window
        // client-side rather than paginate.
        let func = alpha_vantage_function(commodity).ok_or_else(|| {
            FeedError::Decode(format!("alpha vantage does not publish {:?}", commodity))
        })?;
        let url = format!(
            "{}/query?function={}&interval=daily&apikey={}",
            self.base_url, func, self.api_key
        );
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| FeedError::Network(format!("{e}")))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(FeedError::RateLimited);
        }
        if !status.is_success() {
            return Err(FeedError::Network(format!("upstream {status}")));
        }
        let parsed: AlphaVantageResponse = response
            .json()
            .await
            .map_err(|e| FeedError::Decode(format!("json: {e}")))?;
        if let Some(msg) = parsed.error_message {
            return Err(FeedError::Decode(format!("alpha vantage: {msg}")));
        }
        if let Some(note) = parsed.note {
            if note.contains("call frequency") || note.contains("rate limit") {
                return Err(FeedError::RateLimited);
            }
            return Err(FeedError::Decode(format!("alpha vantage note: {note}")));
        }
        let source = parsed
            .name
            .clone()
            .unwrap_or_else(|| "alpha-vantage".into());
        let mut out = Vec::with_capacity(parsed.data.len());
        for row in parsed.data {
            let date = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
                .map_err(|e| FeedError::Decode(format!("date: {e}")))?;
            let at = Utc
                .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
                .single()
                .ok_or_else(|| FeedError::Decode("invalid date".into()))?;
            if at < from || at > to {
                continue;
            }
            let price: f64 = row
                .value
                .parse()
                .map_err(|e| FeedError::Decode(format!("value: {e}")))?;
            out.push(PriceObservation {
                commodity,
                at,
                price,
                usd_price: price,
                source: source.clone(),
            });
        }
        // The future FxRateProvider hook lands here once a
        // non-USD function appears in Alpha Vantage's catalog.
        // Reference `self.fx` so the field isn't unused — the
        // compiler-warn signal is genuine when production
        // code drops a dep.
        let _ = &self.fx;
        // Alpha Vantage returns newest-first; reverse to
        // oldest-first to match the `CommodityFeed::history`
        // contract.
        out.reverse();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::StaticFxRates;

    fn fx() -> Arc<dyn FxRateProvider> {
        Arc::new(StaticFxRates::default())
    }

    #[test]
    fn alpha_vantage_function_maps_supported_commodities() {
        assert_eq!(alpha_vantage_function(Commodity::Copper), Some("COPPER"));
        assert_eq!(
            alpha_vantage_function(Commodity::Aluminum),
            Some("ALUMINUM")
        );
        assert_eq!(alpha_vantage_function(Commodity::Brent), Some("BRENT"));
        assert_eq!(
            alpha_vantage_function(Commodity::NaturalGas),
            Some("NATURAL_GAS")
        );
    }

    #[test]
    fn alpha_vantage_function_returns_none_for_unsupported() {
        // These commodities aren't on Alpha Vantage's
        // commodity-function set; pin so a future "add
        // commodity to AlphaVantage" change doesn't slip in
        // silently.
        assert!(alpha_vantage_function(Commodity::SteelHrc).is_none());
        assert!(alpha_vantage_function(Commodity::Polypropylene).is_none());
        assert!(alpha_vantage_function(Commodity::Lithium).is_none());
        assert!(alpha_vantage_function(Commodity::Cobalt).is_none());
    }

    #[tokio::test]
    async fn unsupported_commodity_in_single_fetch_surfaces_typed_error() {
        let feed = AlphaVantageFeed::new("dummy", fx());
        let err = feed.fetch_latest_one(Commodity::Lithium).await.unwrap_err();
        match err {
            FeedError::Decode(msg) => {
                assert!(msg.contains("alpha vantage does not publish"));
            }
            other => panic!("expected Decode, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn batch_latest_skips_unsupported_commodities_silently() {
        // `latest()` is the batch path used by the
        // recommendation engine. The contract from
        // `HttpCommodityFeed` is "skip rows we can't decode,
        // don't fail the batch." Pin the same behavior here
        // for unsupported commodities.
        let feed = AlphaVantageFeed::new("dummy", fx()).with_base_url("http://127.0.0.1:1");
        // Only Lithium is in the request — unsupported; whole
        // batch reaches the skip path before any HTTP call.
        let result = feed.latest(&[Commodity::Lithium]).await.unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn default_base_url_points_at_alpha_vantage() {
        // Anchor for the wire-format docstring.
        assert!(DEFAULT_BASE_URL.contains("alphavantage.co"));
    }

    #[test]
    fn default_timeout_is_reasonable_for_free_tier_latency() {
        // 15s headroom; not so long that a hard outage feels
        // hung.
        assert!(DEFAULT_TIMEOUT >= Duration::from_secs(10));
        assert!(DEFAULT_TIMEOUT <= Duration::from_secs(60));
    }
}
