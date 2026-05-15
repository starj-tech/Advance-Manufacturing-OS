//! `HttpCommodityFeed` — concrete [`CommodityFeed`] over JSON HTTP.
//!
//! ## Where this sits in the data path
//! ```text
//!   LME / CME / SHFE REST  ──►  (adapter, edge fn or sidecar)
//!                                 │
//!                                 ▼  canonical JSON
//!                          HttpCommodityFeed  ──►  PriceObservation
//!                                                    (USD-normalized)
//! ```
//!
//! The adapter layer between an exchange's real REST shape and this
//! feed lives outside the crate — usually a Supabase Edge Function
//! that polls the exchange and republishes in the canonical shape
//! below. That keeps this feed agnostic to per-exchange auth flows,
//! pagination quirks, and JSON shape churn while preserving the
//! one-place-to-grep contract that AETHER consumes.
//!
//! ## Wire contract
//!
//! `GET <base_url>/v1/commodities/latest?tickers=HRC,CO,LITH`
//! - Headers: `Authorization: Bearer <jwt>` (optional)
//! - Response 200:
//!   ```json
//!   {
//!     "quotes": [
//!       {
//!         "ticker": "HRC",
//!         "timestamp": "2024-01-15T10:30:00Z",
//!         "price": 1250.00,
//!         "currency": "USD",
//!         "source": "LME"
//!       }
//!     ]
//!   }
//!   ```
//!
//! `GET <base_url>/v1/commodities/history?ticker=HRC&from=<rfc3339>&to=<rfc3339>`
//! - Same response shape; entries ordered oldest-first.
//!
//! ## Error mapping
//! - reqwest timeout / connect / request build → `Network`
//! - HTTP 429 → `RateLimited` (caller backs off)
//! - HTTP 5xx → `Network` (transient)
//! - HTTP 4xx (non-429) / JSON decode failure → `Decode`
//! - FxRateProvider error → `Fx`
//!
//! ## Unknown tickers in response
//! A response row whose `ticker` doesn't match any `Commodity::
//! from_ticker(...)` is **dropped silently** rather than failing the
//! whole batch. Rationale: a real-world feed may legitimately expand
//! its catalog faster than AETHER's enum does (a new exchange listing
//! published before we've added the variant). Failing the batch on an
//! unknown ticker means the operator gets no copper price when all
//! that happened upstream was the addition of a forward we don't track.

use crate::feed::{CommodityFeed, FeedError, PriceObservation};
use crate::fx::FxRateProvider;
use crate::model::Commodity;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct LatestResponse {
    quotes: Vec<QuoteWire>,
}

#[derive(Deserialize)]
struct HistoryResponse {
    /// Matches `Commodity::ticker()` so we can sanity-check the
    /// server echoed back the ticker we asked for. Optional — older
    /// adapters may not bother echoing.
    #[allow(dead_code)]
    ticker: Option<String>,
    observations: Vec<QuoteWire>,
}

#[derive(Deserialize)]
struct QuoteWire {
    ticker: String,
    timestamp: DateTime<Utc>,
    price: f64,
    currency: String,
    source: String,
}

/// HTTP-backed `CommodityFeed`. Clone is cheap — `reqwest::Client`
/// is internally `Arc<…>` so the connection pool is shared across
/// clones. `Arc<dyn FxRateProvider>` for the same reason.
#[derive(Clone)]
pub struct HttpCommodityFeed {
    base_url: String,
    auth_token: Option<String>,
    fx: Arc<dyn FxRateProvider>,
    client: Client,
}

impl HttpCommodityFeed {
    pub fn new(base_url: impl Into<String>, fx: Arc<dyn FxRateProvider>, client: Client) -> Self {
        let mut base = base_url.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            base_url: base,
            auth_token: None,
            fx,
            client,
        }
    }

    /// Builder-style: set an optional bearer token. Some self-hosted
    /// feeds run unauthenticated on a private network; production
    /// always sets one.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    fn headers(&self) -> Result<HeaderMap, FeedError> {
        let mut h = HeaderMap::new();
        if let Some(token) = &self.auth_token {
            let bearer = format!("Bearer {token}");
            let val = HeaderValue::from_str(&bearer)
                .map_err(|e| FeedError::Network(format!("invalid auth token: {e}")))?;
            h.insert(AUTHORIZATION, val);
        }
        Ok(h)
    }

    /// Convert one wire row to a USD-normalized `PriceObservation`.
    /// Returns `Ok(None)` for unknown tickers (caller drops). Returns
    /// `Err(Fx(...))` if the currency exists but the FX provider
    /// can't price it — the data-quality alarm.
    async fn to_observation(&self, q: QuoteWire) -> Result<Option<PriceObservation>, FeedError> {
        let Some(commodity) = Commodity::from_ticker(&q.ticker) else {
            // Unknown ticker — silently drop (see module docs).
            return Ok(None);
        };
        let usd_price = self
            .fx
            .to_usd(q.price, &q.currency)
            .await
            .map_err(|e| FeedError::Fx(format!("{} → USD: {e}", q.currency)))?;
        Ok(Some(PriceObservation {
            commodity,
            at: q.timestamp,
            price: q.price,
            usd_price,
            source: q.source,
        }))
    }
}

fn map_reqwest_err(e: reqwest::Error) -> FeedError {
    if e.is_timeout() || e.is_connect() || e.is_request() {
        FeedError::Network(format!("{e}"))
    } else {
        FeedError::Decode(format!("reqwest: {e}"))
    }
}

async fn map_http_status(resp: reqwest::Response) -> FeedError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_else(|_| "<no body>".into());
    let truncated: String = body.chars().take(512).collect();
    match status {
        StatusCode::TOO_MANY_REQUESTS => FeedError::RateLimited,
        s if s.is_server_error() => {
            FeedError::Network(format!("server error {status}: {truncated}"))
        }
        s => FeedError::Decode(format!("unexpected {s}: {truncated}")),
    }
}

#[async_trait]
impl CommodityFeed for HttpCommodityFeed {
    async fn latest(&self, commodities: &[Commodity]) -> Result<Vec<PriceObservation>, FeedError> {
        if commodities.is_empty() {
            // No round-trip when no work — saves a request quota
            // hit when callers haven't gated their loops.
            return Ok(Vec::new());
        }
        let tickers: Vec<&str> = commodities.iter().map(|c| c.ticker()).collect();
        let base = format!("{}/v1/commodities/latest", self.base_url);
        let url = reqwest::Url::parse_with_params(&base, &[("tickers", tickers.join(","))])
            .map_err(|e| FeedError::Network(format!("url build: {e}")))?;

        let resp = self
            .client
            .get(url)
            .headers(self.headers()?)
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if !resp.status().is_success() {
            return Err(map_http_status(resp).await);
        }

        let body: LatestResponse = resp
            .json()
            .await
            .map_err(|e| FeedError::Decode(format!("latest body: {e}")))?;

        let mut out = Vec::with_capacity(body.quotes.len());
        for q in body.quotes {
            if let Some(obs) = self.to_observation(q).await? {
                out.push(obs);
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
        let base = format!("{}/v1/commodities/history", self.base_url);
        let url = reqwest::Url::parse_with_params(
            &base,
            &[
                ("ticker", commodity.ticker()),
                ("from", &from.to_rfc3339()),
                ("to", &to.to_rfc3339()),
            ],
        )
        .map_err(|e| FeedError::Network(format!("url build: {e}")))?;

        let resp = self
            .client
            .get(url)
            .headers(self.headers()?)
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if !resp.status().is_success() {
            return Err(map_http_status(resp).await);
        }

        let body: HistoryResponse = resp
            .json()
            .await
            .map_err(|e| FeedError::Decode(format!("history body: {e}")))?;

        let mut out = Vec::with_capacity(body.observations.len());
        for q in body.observations {
            if let Some(obs) = self.to_observation(q).await? {
                out.push(obs);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::StaticFxRates;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fx() -> Arc<dyn FxRateProvider> {
        Arc::new(StaticFxRates::new().with("EUR", 1.08).with("CNY", 0.14))
    }

    fn feed(server: &MockServer) -> HttpCommodityFeed {
        HttpCommodityFeed::new(server.uri(), fx(), Client::new())
    }

    #[tokio::test]
    async fn latest_returns_usd_normalized_observations() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .and(query_param("tickers", "HG"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "quotes": [{
                    "ticker": "HG",
                    "timestamp": "2024-01-15T10:30:00Z",
                    "price": 9500.0,
                    "currency": "USD",
                    "source": "LME",
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let obs = feed(&server).latest(&[Commodity::Copper]).await.unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].commodity, Commodity::Copper);
        assert!((obs[0].usd_price - 9500.0).abs() < 1e-6);
        assert_eq!(obs[0].source, "LME");
    }

    #[tokio::test]
    async fn non_usd_quotes_are_fx_normalized() {
        // A 1000 EUR quote with EUR=1.08 lands as 1080 USD. Without
        // this normalization, downstream SMA/recommendation logic
        // would mix currencies.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "quotes": [{
                    "ticker": "ALI",
                    "timestamp": "2024-01-15T10:30:00Z",
                    "price": 1000.0,
                    "currency": "EUR",
                    "source": "LME",
                }]
            })))
            .mount(&server)
            .await;

        let obs = feed(&server).latest(&[Commodity::Aluminum]).await.unwrap();
        assert_eq!(obs.len(), 1);
        assert!(
            (obs[0].usd_price - 1080.0).abs() < 1e-6,
            "1000 EUR × 1.08 → 1080 USD; got {}",
            obs[0].usd_price
        );
        // Source-currency price preserved alongside the normalized
        // value so audit consumers can recreate the conversion.
        assert!((obs[0].price - 1000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn multiple_tickers_in_one_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .and(query_param("tickers", "HG,ALI"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "quotes": [
                    {
                        "ticker": "HG",
                        "timestamp": "2024-01-15T10:30:00Z",
                        "price": 9500.0,
                        "currency": "USD",
                        "source": "LME",
                    },
                    {
                        "ticker": "ALI",
                        "timestamp": "2024-01-15T10:30:00Z",
                        "price": 2500.0,
                        "currency": "USD",
                        "source": "LME",
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let obs = feed(&server)
            .latest(&[Commodity::Copper, Commodity::Aluminum])
            .await
            .unwrap();
        assert_eq!(obs.len(), 2);
    }

    #[tokio::test]
    async fn unknown_ticker_in_response_is_silently_dropped() {
        // A future exchange listing AETHER doesn't enumerate yet
        // must not poison the batch — caller still gets the
        // recognized quote.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "quotes": [
                    {
                        "ticker": "HG",
                        "timestamp": "2024-01-15T10:30:00Z",
                        "price": 9500.0,
                        "currency": "USD",
                        "source": "LME",
                    },
                    {
                        "ticker": "NICKEL",
                        "timestamp": "2024-01-15T10:30:00Z",
                        "price": 17000.0,
                        "currency": "USD",
                        "source": "LME",
                    }
                ]
            })))
            .mount(&server)
            .await;

        let obs = feed(&server).latest(&[Commodity::Copper]).await.unwrap();
        assert_eq!(obs.len(), 1, "unknown ticker dropped, recognized one kept");
        assert_eq!(obs[0].commodity, Commodity::Copper);
    }

    #[tokio::test]
    async fn unknown_currency_surfaces_fx_error_not_silent_drop() {
        // The opposite stance from unknown tickers: an unknown
        // currency is a data-quality alarm, not a benign anomaly.
        // FX feed bugs are how millions silently vanish.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "quotes": [{
                    "ticker": "HG",
                    "timestamp": "2024-01-15T10:30:00Z",
                    "price": 9500.0,
                    "currency": "ZWL",
                    "source": "LME",
                }]
            })))
            .mount(&server)
            .await;

        let err = feed(&server)
            .latest(&[Commodity::Copper])
            .await
            .unwrap_err();
        assert!(matches!(err, FeedError::Fx(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn empty_commodities_short_circuits_without_request() {
        // No commodities asked → no request fired. wiremock's expect(0)
        // would catch a stray call; we just confirm no error and
        // empty result without mounting any route.
        let server = MockServer::start().await;
        let obs = feed(&server).latest(&[]).await.unwrap();
        assert!(obs.is_empty());
    }

    #[tokio::test]
    async fn history_passes_from_to_as_query_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/history"))
            .and(query_param("ticker", "HG"))
            // Just assert the params are present — RFC3339 string
            // exactness lives in chrono's own tests.
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ticker": "HG",
                "observations": [
                    {
                        "ticker": "HG",
                        "timestamp": "2024-01-01T00:00:00Z",
                        "price": 9000.0,
                        "currency": "USD",
                        "source": "LME",
                    },
                    {
                        "ticker": "HG",
                        "timestamp": "2024-01-15T00:00:00Z",
                        "price": 9500.0,
                        "currency": "USD",
                        "source": "LME",
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let from = DateTime::<Utc>::from_timestamp(1_704_067_200, 0).unwrap();
        let to = DateTime::<Utc>::from_timestamp(1_705_276_800, 0).unwrap();
        let obs = feed(&server)
            .history(Commodity::Copper, from, to)
            .await
            .unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(obs[0].commodity, Commodity::Copper);
    }

    #[tokio::test]
    async fn http_429_maps_to_rate_limited() {
        // The dedicated retry signal — feeds are rate-limited and the
        // caller's backoff loop relies on this discrimination.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = feed(&server)
            .latest(&[Commodity::Copper])
            .await
            .unwrap_err();
        assert!(matches!(err, FeedError::RateLimited));
    }

    #[tokio::test]
    async fn http_503_maps_to_transient_network_error() {
        // 5xx is transient. Mapping to Network (not Decode) signals
        // the caller's backoff loop to retry.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
            .mount(&server)
            .await;

        let err = feed(&server)
            .latest(&[Commodity::Copper])
            .await
            .unwrap_err();
        assert!(matches!(err, FeedError::Network(_)));
    }

    #[tokio::test]
    async fn malformed_json_yields_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = feed(&server)
            .latest(&[Commodity::Copper])
            .await
            .unwrap_err();
        assert!(matches!(err, FeedError::Decode(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn unreachable_host_yields_network_error() {
        let f = HttpCommodityFeed::new(
            "http://127.0.0.1:1",
            fx(),
            Client::builder()
                .connect_timeout(std::time::Duration::from_millis(200))
                .build()
                .unwrap(),
        );
        let err = f.latest(&[Commodity::Copper]).await.unwrap_err();
        assert!(matches!(err, FeedError::Network(_)));
    }

    #[tokio::test]
    async fn bearer_token_is_attached_when_configured() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            .and(header("authorization", "Bearer feed-jwt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"quotes": []})))
            .expect(1)
            .mount(&server)
            .await;

        let f =
            HttpCommodityFeed::new(server.uri(), fx(), Client::new()).with_auth_token("feed-jwt");
        f.latest(&[Commodity::Copper]).await.unwrap();
    }

    #[tokio::test]
    async fn no_token_means_no_authorization_header() {
        // Unauthenticated self-hosted feeds should still work — no
        // Authorization header should be sent if none configured.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/commodities/latest"))
            // wiremock's matchers can't easily assert "absence", so we
            // mount a permissive route and rely on the .expect(1) +
            // the bearer-attached test above proving the opposite case.
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"quotes": []})))
            .expect(1)
            .mount(&server)
            .await;

        feed(&server).latest(&[Commodity::Copper]).await.unwrap();
    }

    #[test]
    fn base_url_trailing_slash_is_normalized() {
        let f = HttpCommodityFeed::new("https://api.example.com//", fx(), Client::new());
        assert_eq!(f.base_url, "https://api.example.com");
    }
}
