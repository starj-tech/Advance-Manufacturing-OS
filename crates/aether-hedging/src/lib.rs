//! Global Supply Chain Hedging.
//!
//! Treats raw materials as exposure to underlying commodities (steel,
//! aluminum, copper, polypropylene, brent). Continuous price feeds from
//! LME / CME / ICE land in `commodity_prices`; the recommendation engine
//! produces actionable signals: "buy 6-month stock now — projected save
//! 15%", "hedge with futures contract X", "delay purchase 2 weeks".
//!
//! ## Skeleton scope
//! Trait surfaces, types, and a deterministic moving-average
//! recommendation. ML / time-series forecasts ship in PR #7 alongside
//! the analytics module.

pub mod feed;
pub mod model;
pub mod recommend;

pub use feed::{CommodityFeed, FeedError, PriceObservation};
pub use model::{Commodity, MaterialExposure};
pub use recommend::{recommend, HedgeAction, HedgeRecommendation, MovingAvgWindow};
