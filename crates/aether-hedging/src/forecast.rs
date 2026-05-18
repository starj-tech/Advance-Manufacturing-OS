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

/// Exponentially-Weighted Moving Average forecast.
///
/// ## Why EWMA alongside SMA
/// SMA weights every in-window sample equally — a price from 30
/// days ago counts as much as yesterday's. That's noise-tolerant
/// but slow to respond when conditions shift. EWMA decays
/// older samples by a factor `α ∈ (0, 1]` per step, so it
/// tracks the current regime faster while still smoothing
/// short-term noise.
///
/// The recurrence:
///
///     m_t = α · x_t + (1 - α) · m_{t-1}
///     v_t = α · (x_t - m_t)² + (1 - α) · v_{t-1}
///
/// where `m_t` is the running mean (the point forecast) and
/// `v_t` is the EW variance. The 1-σ band is `[m - √v, m + √v]`
/// — gives a real uncertainty estimate, unlike SMA which
/// collapses the band to the point.
///
/// This is the canonical industrial signal-processing model
/// for vibration / thermal / current sensors in
/// predictive-maintenance pipelines and serves as the
/// non-LSTM analytics path until the V7 LSTM lands.
pub struct EwmaForecast {
    /// Smoothing factor 0 < α ≤ 1. Higher α = more weight on
    /// recent samples. Default 0.3 (≈ 6-sample effective
    /// window) is a reasonable starting point for daily
    /// commodity series; vibration sensors at 1 Hz typically
    /// use 0.05–0.1.
    pub alpha: f64,
    /// Confidence-band scale in standard deviations. 1.0
    /// produces a 1-σ band (~68% coverage under normality);
    /// 2.0 produces a 2-σ band (~95%). The recommendation
    /// engine consumes the band; production callers typically
    /// keep this at 1.0 and gate decisions on confidence.
    pub band_sigma: f64,
}

/// Default EWMA smoothing factor. Picked for daily commodity
/// series; predictive-maintenance impls override via
/// `EwmaForecast::with_alpha`.
pub const DEFAULT_EWMA_ALPHA: f64 = 0.3;

/// Default band sigma (1-σ ≈ 68% coverage).
pub const DEFAULT_BAND_SIGMA: f64 = 1.0;

impl EwmaForecast {
    pub fn new() -> Self {
        Self {
            alpha: DEFAULT_EWMA_ALPHA,
            band_sigma: DEFAULT_BAND_SIGMA,
        }
    }

    /// Override the smoothing factor. Clamps to `(0, 1]` —
    /// `α = 0` would freeze the mean forever, which is never
    /// what a forecaster wants; values above 1 are nonsensical.
    pub fn with_alpha(mut self, alpha: f64) -> Self {
        // Tightest possible positive lower bound — anything
        // smaller is effectively "no learning."
        self.alpha = alpha.clamp(f64::EPSILON, 1.0);
        self
    }

    /// Override the confidence-band width in σ units.
    pub fn with_band_sigma(mut self, sigma: f64) -> Self {
        self.band_sigma = sigma.max(0.0);
        self
    }
}

impl Default for EwmaForecast {
    fn default() -> Self {
        Self::new()
    }
}

impl ForecastModel for EwmaForecast {
    fn forecast(
        &self,
        commodity: Commodity,
        history: &[PriceObservation],
        now: DateTime<Utc>,
    ) -> Option<Forecast> {
        // Filter by commodity AND by "at or before now" — a
        // forecast for time `now` MUST NOT peek at future
        // samples (the backtest determinism contract).
        let mut series: Vec<&PriceObservation> = history
            .iter()
            .filter(|p| p.commodity == commodity && p.at <= now)
            .collect();
        if series.is_empty() {
            return None;
        }
        // Sort by time ascending — EWMA is order-sensitive.
        series.sort_by_key(|p| p.at);

        let alpha = self.alpha;
        let mut mean = series[0].usd_price;
        let mut var = 0.0;
        for p in series.iter().skip(1) {
            let x = p.usd_price;
            let new_mean = alpha * x + (1.0 - alpha) * mean;
            let dev = x - new_mean;
            var = alpha * dev * dev + (1.0 - alpha) * var;
            mean = new_mean;
        }

        let std = var.sqrt();
        let band = self.band_sigma * std;
        // Confidence ramps with sample count — same heuristic
        // as SMA. EWMA technically converges on any data, but
        // a 2-sample EWMA is hardly trustworthy, so we de-rate
        // until we have a full window's worth.
        let confidence = (series.len() as f64 / FULL_WINDOW_SAMPLES as f64).min(1.0);

        Some(Forecast {
            point: mean,
            lower: mean - band,
            upper: mean + band,
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

    // ---------- EWMA tests (V31) ----------

    #[test]
    fn ewma_empty_history_returns_none() {
        let m = EwmaForecast::new();
        assert!(m.forecast(Commodity::Copper, &[], Utc::now()).is_none());
    }

    #[test]
    fn ewma_constant_series_converges_to_the_constant() {
        // Feed a flat series — EWMA's mean MUST equal the
        // constant exactly (any α, any window). Pin this as
        // the "no spurious drift" property.
        let m = EwmaForecast::new();
        let hist: Vec<_> = (1..=20).map(|d| p(Commodity::Copper, d, 8_500.0)).collect();
        let f = m.forecast(Commodity::Copper, &hist, Utc::now()).unwrap();
        assert!((f.point - 8_500.0).abs() < 1e-6);
        // Constant series has zero variance → collapsed band.
        assert!((f.upper - f.lower).abs() < 1e-6);
    }

    #[test]
    fn ewma_responds_faster_than_sma_to_a_step_change() {
        // The headline property: a step from 100 → 200 lifts
        // EWMA's mean above the SMA's mean (EWMA weights the
        // recent half of the series more). Day-numbers are
        // smaller = more recent (helper subtracts days from
        // now), so we put 200s in the recent half.
        let mut hist = Vec::new();
        for d in (11..=20).rev() {
            hist.push(p(Commodity::Aluminum, d, 100.0));
        }
        for d in (1..=10).rev() {
            hist.push(p(Commodity::Aluminum, d, 200.0));
        }
        let sma = SmaForecast::new(MovingAvgWindow::Days30);
        let ewma = EwmaForecast::new().with_alpha(0.5);
        let f_sma = sma
            .forecast(Commodity::Aluminum, &hist, Utc::now())
            .unwrap();
        let f_ewma = ewma
            .forecast(Commodity::Aluminum, &hist, Utc::now())
            .unwrap();
        // SMA = arithmetic mean of 10×100 + 10×200 = 150.
        assert!((f_sma.point - 150.0).abs() < 1e-6);
        // EWMA at α=0.5 with recent half = 200s should be
        // strictly above 150 (closer to 200).
        assert!(
            f_ewma.point > f_sma.point,
            "EWMA must respond faster than SMA to a step change ({} vs {})",
            f_ewma.point,
            f_sma.point
        );
    }

    #[test]
    fn ewma_band_widens_with_volatility() {
        // Volatile series → larger √var → wider band. Pin
        // this so a regression that drops the variance term
        // surfaces clearly.
        let calm: Vec<_> = (1..=20).map(|d| p(Commodity::Copper, d, 8_500.0)).collect();
        let volatile: Vec<_> = (1..=20)
            .map(|d| {
                let jitter = if d % 2 == 0 { 500.0 } else { -500.0 };
                p(Commodity::Copper, d, 8_500.0 + jitter)
            })
            .collect();
        let m = EwmaForecast::new().with_alpha(0.5);
        let calm_f = m.forecast(Commodity::Copper, &calm, Utc::now()).unwrap();
        let vol_f = m
            .forecast(Commodity::Copper, &volatile, Utc::now())
            .unwrap();
        let calm_width = calm_f.upper - calm_f.lower;
        let vol_width = vol_f.upper - vol_f.lower;
        assert!(vol_width > calm_width);
    }

    #[test]
    fn ewma_alpha_clamps_to_positive_and_at_most_one() {
        // α = 0 would freeze the mean (never updates) — clamp
        // upward. α > 1 is nonsensical — clamp down. Pin both
        // boundaries.
        let lo = EwmaForecast::new().with_alpha(0.0);
        let hi = EwmaForecast::new().with_alpha(2.0);
        assert!(lo.alpha > 0.0);
        assert!(hi.alpha <= 1.0);
    }

    #[test]
    fn ewma_strict_with_alpha_one_equals_latest_sample() {
        // α=1 means "all weight on the newest sample"
        // — degenerate, but a useful boundary check that
        // proves the recurrence implementation. The mean
        // should equal the last (most recent) usd_price.
        let m = EwmaForecast::new().with_alpha(1.0);
        let hist = vec![
            p(Commodity::Copper, 5, 100.0),
            p(Commodity::Copper, 3, 200.0),
            p(Commodity::Copper, 1, 300.0),
        ];
        let f = m.forecast(Commodity::Copper, &hist, Utc::now()).unwrap();
        assert!((f.point - 300.0).abs() < 1e-6);
    }

    #[test]
    fn ewma_does_not_peek_at_future_samples() {
        // Determinism + backtest contract: a forecast for
        // time `t` MUST NOT include samples with at > t.
        let hist = vec![
            p(Commodity::Copper, 30, 100.0),
            p(Commodity::Copper, 1, 999.0), // "yesterday"
        ];
        let m = EwmaForecast::new().with_alpha(1.0);
        // Ask for a forecast 60 days ago — the "yesterday"
        // sample is in the future for that anchor.
        let then = Utc::now() - Duration::days(60);
        let f = m.forecast(Commodity::Copper, &hist, then);
        // Only the 30-days-ago sample is at-or-before t;
        // but t is 60 days ago so even that's future. Forecast
        // is None.
        assert!(f.is_none());
    }

    #[test]
    fn ewma_determinism_across_calls() {
        // Same inputs → byte-identical output.
        let m = EwmaForecast::new();
        let hist: Vec<_> = (1..=15)
            .map(|d| p(Commodity::Lithium, d, 50_000.0 + d as f64))
            .collect();
        let now = Utc::now();
        let f1 = m.forecast(Commodity::Lithium, &hist, now).unwrap();
        let f2 = m.forecast(Commodity::Lithium, &hist, now).unwrap();
        assert_eq!(f1, f2);
    }

    #[test]
    fn ewma_confidence_ramps_with_sample_count() {
        // Same as SMA: more samples → higher confidence.
        let m = EwmaForecast::new();
        let few: Vec<_> = (1..=2).map(|d| p(Commodity::Cobalt, d, 30_000.0)).collect();
        let many: Vec<_> = (1..=20)
            .map(|d| p(Commodity::Cobalt, d, 30_000.0))
            .collect();
        let f_few = m.forecast(Commodity::Cobalt, &few, Utc::now()).unwrap();
        let f_many = m.forecast(Commodity::Cobalt, &many, Utc::now()).unwrap();
        assert!(f_many.confidence > f_few.confidence);
        assert!((f_many.confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ewma_filters_by_commodity() {
        // Strict commodity filtering — same property the SMA
        // upholds; pin for EWMA too so an aluminum sample
        // can't pollute a copper forecast.
        let m = EwmaForecast::new();
        let hist = vec![
            p(Commodity::Aluminum, 5, 2_000.0),
            p(Commodity::Aluminum, 3, 2_500.0),
        ];
        assert!(m.forecast(Commodity::Copper, &hist, Utc::now()).is_none());
    }
}
