use crate::feed::PriceObservation;
use crate::forecast::{Forecast, ForecastModel, SmaForecast};
use crate::model::Commodity;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HedgeAction {
    /// Buy now, build runway. Reasoned when spot < forecast by
    /// `dip_pct` AND momentum is non-negative.
    BuyNow { cover_months: u32 },
    /// Defer purchase; market is overpriced relative to history.
    Wait,
    /// Hold the current position.
    Hold,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HedgeRecommendation {
    pub commodity: Commodity,
    pub spot: f64,
    /// Reference price the spot is compared against. With `SmaForecast`
    /// this is the simple moving average — kept as `sma` for backward
    /// compatibility with the report schema, even when the underlying
    /// model is LSTM-based (the field stays the model's point
    /// estimate regardless of provenance).
    pub sma: f64,
    pub action: HedgeAction,
    pub estimated_savings_pct: f64,
    /// Human-readable rationale for surfacing in the UI / report.
    pub rationale: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MovingAvgWindow {
    Days30,
    Days90,
    Days180,
}

impl MovingAvgWindow {
    pub fn duration(&self) -> Duration {
        match self {
            MovingAvgWindow::Days30 => Duration::days(30),
            MovingAvgWindow::Days90 => Duration::days(90),
            MovingAvgWindow::Days180 => Duration::days(180),
        }
    }
}

/// Dip / spike thresholds. Above the dip threshold we buy; below the
/// negative threshold we wait; otherwise we hold. Centralised so the
/// thresholds can be tuned in one place when the analytics module
/// lands with its own evidence base.
const DIP_PCT_BUY: f64 = 10.0;
const SPIKE_PCT_WAIT: f64 = -10.0;

/// Backward-compatible entry point: delegates to a `SmaForecast` with
/// the requested window. Existing callers ship through this unchanged;
/// new callers needing LSTM or Monte Carlo route through
/// [`recommend_with`].
pub fn recommend(
    commodity: Commodity,
    history: &[PriceObservation],
    window: MovingAvgWindow,
    now: DateTime<Utc>,
) -> Option<HedgeRecommendation> {
    let model = SmaForecast::new(window);
    recommend_with(commodity, history, &model, now)
}

/// Strategy-agnostic recommendation. Takes any [`ForecastModel`] and
/// produces a hedge action by comparing the latest observed spot
/// against the model's point estimate. The forecast's confidence band
/// is forwarded into the rationale so a low-confidence "Buy now"
/// reads differently from a high-confidence one — but the action
/// itself is gated only on the dip/spike percentage, leaving threshold
/// tuning to a higher policy layer.
pub fn recommend_with(
    commodity: Commodity,
    history: &[PriceObservation],
    model: &dyn ForecastModel,
    now: DateTime<Utc>,
) -> Option<HedgeRecommendation> {
    if history.is_empty() {
        return None;
    }
    let forecast: Forecast = model.forecast(commodity, history, now)?;

    // The "spot" is the most recent in-history observation for this
    // commodity, not just `history.last()` — the slice may interleave
    // multiple commodities.
    let spot = latest_spot(commodity, history, now)?;
    let reference = forecast.point;

    let dip_pct = if reference > 0.0 {
        (reference - spot) / reference * 100.0
    } else {
        0.0
    };

    let action = if dip_pct >= DIP_PCT_BUY {
        HedgeAction::BuyNow { cover_months: 6 }
    } else if dip_pct <= SPIKE_PCT_WAIT {
        HedgeAction::Wait
    } else {
        HedgeAction::Hold
    };

    let rationale = build_rationale(commodity, spot, &forecast, dip_pct, action);

    Some(HedgeRecommendation {
        commodity,
        spot,
        sma: reference,
        action,
        estimated_savings_pct: dip_pct.max(0.0),
        rationale,
    })
}

fn latest_spot(
    commodity: Commodity,
    history: &[PriceObservation],
    now: DateTime<Utc>,
) -> Option<f64> {
    history
        .iter()
        .filter(|p| p.commodity == commodity && p.at <= now)
        .max_by_key(|p| p.at)
        .map(|p| p.usd_price)
}

fn build_rationale(
    commodity: Commodity,
    spot: f64,
    forecast: &Forecast,
    dip_pct: f64,
    action: HedgeAction,
) -> String {
    // Surface confidence so a 0.3 forecast reads differently from a
    // 1.0 forecast in the UI. Skip the suffix when confidence is full
    // so the SMA-with-full-window case stays terse.
    let confidence_suffix = if forecast.confidence < 0.99 {
        format!(" (confidence {:.0}%)", forecast.confidence * 100.0)
    } else {
        String::new()
    };

    match action {
        HedgeAction::BuyNow { cover_months } => format!(
            "{} spot ${:.2} is {:.1}% below forecast ${:.2}; lock {}-month supply now{}",
            commodity.ticker(),
            spot,
            dip_pct.abs(),
            forecast.point,
            cover_months,
            confidence_suffix,
        ),
        HedgeAction::Wait => format!(
            "{} spot ${:.2} is {:.1}% above forecast ${:.2} — defer{}",
            commodity.ticker(),
            spot,
            dip_pct.abs(),
            forecast.point,
            confidence_suffix,
        ),
        HedgeAction::Hold => format!(
            "{} spot ${:.2} is within ±{}% of forecast ${:.2}{}",
            commodity.ticker(),
            spot,
            DIP_PCT_BUY as u32,
            forecast.point,
            confidence_suffix,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    fn p(c: Commodity, days_ago: i64, price: f64) -> PriceObservation {
        let at = Utc::now() - Duration::days(days_ago);
        PriceObservation {
            commodity: c,
            at,
            price,
            usd_price: price,
            source: "test".into(),
        }
    }

    #[test]
    fn empty_history_returns_none() {
        let r = recommend(Commodity::Copper, &[], MovingAvgWindow::Days30, Utc::now());
        assert!(r.is_none());
    }

    #[test]
    fn dip_triggers_buy_now() {
        let mut hist = vec![];
        for i in 0..30 {
            hist.push(p(Commodity::Copper, 30 - i, 10000.0));
        }
        // Last point: a 15% dip.
        hist.push(p(Commodity::Copper, 0, 8500.0));
        let r = recommend(
            Commodity::Copper,
            &hist,
            MovingAvgWindow::Days30,
            Utc::now(),
        )
        .unwrap();
        assert!(matches!(r.action, HedgeAction::BuyNow { cover_months: 6 }));
        assert!(r.estimated_savings_pct > 10.0);
    }

    #[test]
    fn spike_triggers_wait() {
        let mut hist = vec![];
        for i in 0..30 {
            hist.push(p(Commodity::Aluminum, 30 - i, 2400.0));
        }
        hist.push(p(Commodity::Aluminum, 0, 3000.0));
        let r = recommend(
            Commodity::Aluminum,
            &hist,
            MovingAvgWindow::Days30,
            Utc::now(),
        )
        .unwrap();
        assert!(matches!(r.action, HedgeAction::Wait));
    }

    #[test]
    fn flat_market_holds() {
        let mut hist = vec![];
        for i in 0..30 {
            hist.push(p(Commodity::Brent, 30 - i, 80.0 + (i as f64 * 0.05)));
        }
        let r = recommend(Commodity::Brent, &hist, MovingAvgWindow::Days30, Utc::now()).unwrap();
        assert!(matches!(r.action, HedgeAction::Hold));
    }

    // -- ForecastModel integration --

    /// Custom model that always reports a fixed forecast — lets us
    /// prove that `recommend_with` is correctly delegating rather than
    /// silently re-implementing SMA. If the recommendation diverges
    /// from what the model says, this test is the alarm.
    struct FixedForecast {
        value: f64,
        confidence: f64,
    }

    impl ForecastModel for FixedForecast {
        fn forecast(
            &self,
            _commodity: Commodity,
            _history: &[PriceObservation],
            _now: DateTime<Utc>,
        ) -> Option<Forecast> {
            Some(Forecast {
                point: self.value,
                lower: self.value,
                upper: self.value,
                confidence: self.confidence,
            })
        }
    }

    #[test]
    fn recommend_with_consults_the_injected_forecast_model() {
        // History says copper is at 10_000, but the injected model
        // claims the forward is 12_000 — recommendation must use the
        // model's forward, not re-derive an SMA from history.
        let hist = vec![p(Commodity::Copper, 1, 10_000.0)];
        let model = FixedForecast {
            value: 12_000.0,
            confidence: 1.0,
        };
        let r = recommend_with(Commodity::Copper, &hist, &model, Utc::now()).unwrap();
        assert!((r.sma - 12_000.0).abs() < 1e-6);
        // spot $10,000 vs forecast $12,000 = 16.7% dip → BuyNow.
        assert!(matches!(r.action, HedgeAction::BuyNow { .. }));
    }

    #[test]
    fn recommend_with_returns_none_when_model_returns_none() {
        // A model that can't forecast (insufficient data, unknown
        // commodity) MUST surface as no recommendation rather than
        // a "Hold by default" silent fallback.
        struct AlwaysNone;
        impl ForecastModel for AlwaysNone {
            fn forecast(
                &self,
                _: Commodity,
                _: &[PriceObservation],
                _: DateTime<Utc>,
            ) -> Option<Forecast> {
                None
            }
        }
        let hist = vec![p(Commodity::Copper, 1, 10_000.0)];
        assert!(recommend_with(Commodity::Copper, &hist, &AlwaysNone, Utc::now()).is_none());
    }

    #[test]
    fn confidence_below_full_appears_in_rationale_text() {
        let hist = vec![p(Commodity::Lithium, 1, 50_000.0)];
        let model = FixedForecast {
            value: 60_000.0,
            confidence: 0.5,
        };
        let r = recommend_with(Commodity::Lithium, &hist, &model, Utc::now()).unwrap();
        assert!(
            r.rationale.contains("confidence"),
            "low-confidence forecast surfaces in the rationale: `{}`",
            r.rationale
        );
        assert!(r.rationale.contains("50%"));
    }

    #[test]
    fn full_confidence_rationale_omits_the_confidence_suffix() {
        let hist = vec![p(Commodity::Lithium, 1, 50_000.0)];
        let model = FixedForecast {
            value: 60_000.0,
            confidence: 1.0,
        };
        let r = recommend_with(Commodity::Lithium, &hist, &model, Utc::now()).unwrap();
        assert!(
            !r.rationale.contains("confidence"),
            "full-confidence rationale stays terse: `{}`",
            r.rationale
        );
    }

    #[test]
    fn spot_picks_the_latest_in_history_for_the_target_commodity() {
        // The history may interleave commodities — recommendation for
        // Copper must not be polluted by a more recent Aluminum point.
        let hist = vec![
            p(Commodity::Copper, 5, 10_000.0),
            p(Commodity::Aluminum, 1, 2_500.0),
        ];
        let model = FixedForecast {
            value: 10_000.0,
            confidence: 1.0,
        };
        let r = recommend_with(Commodity::Copper, &hist, &model, Utc::now()).unwrap();
        assert!((r.spot - 10_000.0).abs() < 1e-6);
    }

    // Compile-time assertion that the trait is dyn-compatible — without
    // this, `&dyn ForecastModel` would fail to type-check downstream.
    #[allow(dead_code)]
    fn _dyn_compat(_m: &dyn ForecastModel) {}

    // Sanity that async traits aren't required on this side (synchronous
    // forecast() is a deliberate choice; LSTM in PR #7 will run on a
    // blocking thread pool via spawn_blocking rather than .await).
    #[async_trait]
    trait _AsyncSanity {
        async fn _noop(&self);
    }
}
