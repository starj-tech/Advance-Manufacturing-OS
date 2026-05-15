//! Forecast abstraction — trait surface that the recommendation engine
//! consumes so SMA, EMA, LSTM, and Monte-Carlo variants are
//! interchangeable.
//!
//! ## Why a trait
//! `recommend()` originally hard-coded a 30/90/180-day simple moving
//! average. The PR #7 analytics module needs to drop in LSTM + Monte
//! Carlo without rewriting the recommendation logic. The trait makes
//! that drop-in possible:
//!
//!   let model: Box<dyn ForecastModel> = match config.strategy {
//!       Strategy::Sma  => Box::new(SmaForecast::new(window)),
//!       Strategy::Lstm => Box::new(LstmForecast::new(weights)),  // PR #7
//!   };
//!   recommend_with(commodity, history, model.as_ref(), now);
//!
//! ## Forecast shape
//! Every model returns a [`Forecast`] with a point estimate plus a
//! lower/upper confidence band. Bands are how Monte Carlo will surface
//! uncertainty; deterministic models (like SMA) collapse them to the
//! point estimate (lower == upper == point). The `confidence` field is
//! the model's self-reported confidence (0.0..=1.0) — callers can gate
//! "Buy now" actions on a minimum confidence threshold once the LSTM
//! variant lands.
//!
//! ## Determinism contract
//! Given the same `history` slice and `now`, two calls to `forecast()`
//! on the same model MUST produce byte-identical Forecast structs.
//! That's what lets the recommendation engine cache/dedupe and what
//! lets backtesting reproduce historical recommendations.

use crate::feed::PriceObservation;
use crate::model::Commodity;
use crate::recommend::MovingAvgWindow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Forecast {
    /// Best-guess forward price (USD, normalized).
    pub point: f64,
    /// Lower bound of the 1-σ confidence band. Equal to `point` for
    /// deterministic models that don't expose uncertainty.
    pub lower: f64,
    /// Upper bound of the 1-σ confidence band. Same caveat as `lower`.
    pub upper: f64,
    /// Self-reported 0.0..=1.0 confidence. SMA returns 1.0 when it
    /// has a full window of data, less when the window is sparse.
    pub confidence: f64,
}

pub trait ForecastModel: Send + Sync {
    /// Produce a forecast for `commodity` given the prior price
    /// series. The `history` is the FULL observation set — the model
    /// is responsible for slicing to its own window. `now` is the
    /// reference time for relative windowing.
    ///
    /// Returns `None` when there is insufficient data to forecast
    /// (e.g. history is empty or has nothing for this commodity).
    /// `recommend()` translates None into "no recommendation".
    fn forecast(
        &self,
        commodity: Commodity,
        history: &[PriceObservation],
        now: DateTime<Utc>,
    ) -> Option<Forecast>;
}

/// Simple Moving Average forecast — average of the in-window USD
/// prices. Deterministic, so the confidence band collapses to the
/// point estimate. Confidence ramps with the number of observations
/// inside the window: 1.0 once we have at least `FULL_WINDOW_SAMPLES`,
/// linearly scaled below that.
pub struct SmaForecast {
    pub window: MovingAvgWindow,
}

/// Number of in-window samples below which SMA confidence is
/// linearly de-rated. Picked so a 30-day window with daily samples
/// hits 1.0 confidence and shorter windows degrade gracefully.
const FULL_WINDOW_SAMPLES: usize = 20;

impl SmaForecast {
    pub fn new(window: MovingAvgWindow) -> Self {
        Self { window }
    }
}

impl ForecastModel for SmaForecast {
    fn forecast(
        &self,
        commodity: Commodity,
        history: &[PriceObservation],
        now: DateTime<Utc>,
    ) -> Option<Forecast> {
        let cutoff = now - self.window.duration();
        let in_window: Vec<&PriceObservation> = history
            .iter()
            .filter(|p| p.commodity == commodity && p.at >= cutoff && p.at <= now)
            .collect();
        if in_window.is_empty() {
            return None;
        }

        let sum: f64 = in_window.iter().map(|p| p.usd_price).sum();
        let mean = sum / in_window.len() as f64;

        // SMA carries no uncertainty band of its own — Monte Carlo
        // bolts that on in PR #7. For now collapse the bounds onto
        // the point estimate so downstream consumers can rely on
        // `lower <= point <= upper` without branching.
        let confidence = (in_window.len() as f64 / FULL_WINDOW_SAMPLES as f64).min(1.0);

        Some(Forecast {
            point: mean,
            lower: mean,
            upper: mean,
            confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn p(c: Commodity, days_ago: i64, price: f64) -> PriceObservation {
        PriceObservation {
            commodity: c,
            at: Utc::now() - Duration::days(days_ago),
            price,
            usd_price: price,
            source: "test".into(),
        }
    }

    #[test]
    fn empty_history_returns_none() {
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        assert!(m.forecast(Commodity::Copper, &[], Utc::now()).is_none());
    }

    #[test]
    fn history_for_other_commodity_returns_none() {
        // Strict commodity filtering — a copper forecast must not be
        // contaminated by aluminum prices, even if they're in the
        // same window.
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        let hist = vec![p(Commodity::Aluminum, 5, 2500.0)];
        assert!(m.forecast(Commodity::Copper, &hist, Utc::now()).is_none());
    }

    #[test]
    fn out_of_window_observations_are_ignored() {
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        let hist = vec![
            p(Commodity::Copper, 5, 10_000.0),
            // Way outside the 30-day window — must NOT skew the mean.
            p(Commodity::Copper, 365, 50_000.0),
        ];
        let f = m.forecast(Commodity::Copper, &hist, Utc::now()).unwrap();
        assert!((f.point - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn mean_matches_arithmetic_mean_of_in_window_prices() {
        let m = SmaForecast::new(MovingAvgWindow::Days90);
        let hist = vec![
            p(Commodity::Brent, 10, 80.0),
            p(Commodity::Brent, 20, 82.0),
            p(Commodity::Brent, 30, 78.0),
            p(Commodity::Brent, 60, 90.0),
        ];
        // Mean = (80 + 82 + 78 + 90) / 4 = 82.5
        let f = m.forecast(Commodity::Brent, &hist, Utc::now()).unwrap();
        assert!((f.point - 82.5).abs() < 1e-6);
    }

    #[test]
    fn deterministic_model_collapses_uncertainty_band() {
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        let hist: Vec<_> = (1..=20).map(|d| p(Commodity::Copper, d, 9000.0)).collect();
        let f = m.forecast(Commodity::Copper, &hist, Utc::now()).unwrap();
        // SMA carries no band — bounds collapse onto the point. The
        // `lower <= point <= upper` invariant still holds, but they're
        // all equal. PR #7's Monte Carlo will broaden these.
        assert_eq!(f.lower, f.point);
        assert_eq!(f.upper, f.point);
        assert!(f.lower <= f.point);
        assert!(f.point <= f.upper);
    }

    #[test]
    fn confidence_ramps_with_sample_count_and_caps_at_one() {
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        // 5 samples: confidence ≈ 5/20 = 0.25
        let sparse: Vec<_> = (1..=5).map(|d| p(Commodity::Copper, d, 9000.0)).collect();
        let f_sparse = m.forecast(Commodity::Copper, &sparse, Utc::now()).unwrap();
        assert!((f_sparse.confidence - 0.25).abs() < 1e-6);

        // Full window: confidence saturates at 1.0
        let full: Vec<_> = (1..=25).map(|d| p(Commodity::Copper, d, 9000.0)).collect();
        let f_full = m.forecast(Commodity::Copper, &full, Utc::now()).unwrap();
        assert!((f_full.confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn determinism_two_calls_produce_identical_forecasts() {
        // The determinism contract — load-bearing for caching and
        // backtest reproducibility.
        let m = SmaForecast::new(MovingAvgWindow::Days30);
        let hist: Vec<_> = (1..=15)
            .map(|d| p(Commodity::Lithium, d, 50_000.0))
            .collect();
        let now = Utc::now();
        let f1 = m.forecast(Commodity::Lithium, &hist, now).unwrap();
        let f2 = m.forecast(Commodity::Lithium, &hist, now).unwrap();
        assert_eq!(f1, f2);
    }

    #[test]
    fn larger_window_includes_more_samples() {
        // Sanity: a 180-day window pulls in the 100-day-old sample
        // that the 30-day window correctly excluded.
        let hist = vec![
            p(Commodity::Cobalt, 10, 30_000.0),
            p(Commodity::Cobalt, 100, 50_000.0),
        ];
        let m30 = SmaForecast::new(MovingAvgWindow::Days30);
        let m180 = SmaForecast::new(MovingAvgWindow::Days180);

        let f30 = m30.forecast(Commodity::Cobalt, &hist, Utc::now()).unwrap();
        let f180 = m180.forecast(Commodity::Cobalt, &hist, Utc::now()).unwrap();
        assert!((f30.point - 30_000.0).abs() < 1e-6);
        assert!((f180.point - 40_000.0).abs() < 1e-6);
    }
}
