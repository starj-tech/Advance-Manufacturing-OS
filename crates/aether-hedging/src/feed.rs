use crate::model::Commodity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriceObservation {
    pub commodity: Commodity,
    pub at: DateTime<Utc>,
    pub price: f64,
    /// USD-equivalent price (some feeds quote in EUR/CNY etc).
    pub usd_price: f64,
    /// Exchange that quoted (e.g. "LME", "CME", "ICE", "SHFE").
    pub source: String,
}

#[derive(Debug, Error)]
pub enum FeedError {
    #[error("network: {0}")]
    Network(String),
    #[error("rate limited")]
    RateLimited,
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

#[async_trait::async_trait]
pub trait CommodityFeed: Send + Sync {
    /// Fetch the latest spot for each requested commodity. Implementations
    /// MUST honor backoff on `RateLimited` — feeds typically allow 60-120
    /// requests per minute.
    async fn latest(&self, commodities: &[Commodity]) -> Result<Vec<PriceObservation>, FeedError>;

    /// Fetch historical prices for backtesting.
    async fn history(
        &self,
        commodity: Commodity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<PriceObservation>, FeedError>;
}
