use crate::feed::PriceObservation;
use crate::model::Commodity;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HedgeAction {
    /// Buy now, build runway. Reasoned when spot < SMA(window) by
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

/// Deterministic moving-average hedge heuristic.
///
/// PR #7's analytics module replaces this with an LSTM forecast + Monte
/// Carlo simulation, but the trait surface remains the same.
pub fn recommend(
    commodity: Commodity,
    history: &[PriceObservation],
    window: MovingAvgWindow,
    now: DateTime<Utc>,
) -> Option<HedgeRecommendation> {
    if history.is_empty() {
        return None;
    }

    let cutoff = now - window.duration();
    let in_window: Vec<&PriceObservation> = history
        .iter()
        .filter(|p| p.commodity == commodity && p.at >= cutoff && p.at <= now)
        .collect();

    if in_window.is_empty() {
        return None;
    }

    let spot = in_window.last().map(|p| p.usd_price).unwrap_or(0.0);
    let sma: f64 = in_window.iter().map(|p| p.usd_price).sum::<f64>() / in_window.len() as f64;

    let dip_pct = if sma > 0.0 {
        (sma - spot) / sma * 100.0
    } else {
        0.0
    };

    let action = if dip_pct >= 10.0 {
        // Significant dip — recommend buying 6-month runway.
        HedgeAction::BuyNow { cover_months: 6 }
    } else if dip_pct <= -10.0 {
        HedgeAction::Wait
    } else {
        HedgeAction::Hold
    };

    let rationale = match action {
        HedgeAction::BuyNow { cover_months } => format!(
            "{} spot ${:.2} is {:.1}% below {}-period average ${:.2}; lock {}-month supply now",
            commodity.ticker(),
            spot,
            dip_pct.abs(),
            in_window.len(),
            sma,
            cover_months
        ),
        HedgeAction::Wait => format!(
            "{} spot ${:.2} is {:.1}% above {}-period average — defer",
            commodity.ticker(),
            spot,
            dip_pct.abs(),
            in_window.len()
        ),
        HedgeAction::Hold => format!(
            "{} spot ${:.2} is within ±10% of average ${:.2}",
            commodity.ticker(),
            spot,
            sma
        ),
    };

    Some(HedgeRecommendation {
        commodity,
        spot,
        sma,
        action,
        estimated_savings_pct: dip_pct.max(0.0),
        rationale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
