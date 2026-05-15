//! FX normalization — convert non-USD commodity quotes to USD.
//!
//! ## Why it lives here
//! LME quotes in USD, but SHFE quotes in CNY and some boutique feeds
//! quote in EUR. Every downstream consumer (recommendation engine,
//! hedge ledger, P&L projection) operates in USD because that's where
//! futures contracts price and where the existing `PriceObservation::
//! usd_price` field assumes its currency. Rather than burying conversion
//! logic inside every `CommodityFeed` implementation, this crate
//! exposes a single `FxRateProvider` trait so:
//!
//!   * One implementation can hit the Supabase `exchange_rates` table
//!     (PR #6 production path — TWAP fixings from a cloud feed).
//!   * Another can hit a third-party FX API in real-time.
//!   * Tests use [`StaticFxRates`] for deterministic round-trips.
//!
//! ## Currency identity
//! Currencies are ISO 4217 alphabetic codes (`"USD"`, `"EUR"`, `"CNY"`,
//! `"IDR"`, …) as `&str` rather than a closed enum. Closed enums would
//! force this crate to know every currency the world adds; the string
//! API delegates "is this a known currency?" to the rate provider,
//! which can answer with a fresh `FxError::UnknownCurrency` and have
//! the answer change with provider data.
//!
//! ## Rate convention
//! `usd_per(code)` returns "how many USD does 1 unit of `code` buy?"
//! `EUR → 1.08` means one euro buys 1.08 dollars. USD itself returns
//! 1.0 by definition.

use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FxError {
    #[error("unknown currency: {0}")]
    UnknownCurrency(String),
    #[error("rate source unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait FxRateProvider: Send + Sync {
    /// Return how many USD one unit of `currency` buys at the
    /// provider's current reference fixing. Implementations should
    /// return Ok(1.0) for `"USD"` without consulting an external
    /// source — the no-op fastpath matters for high-volume feeds.
    async fn usd_per(&self, currency: &str) -> Result<f64, FxError>;

    /// Convenience: convert `amount` in `currency` to USD. Default
    /// impl in terms of `usd_per` — providers usually don't override.
    async fn to_usd(&self, amount: f64, currency: &str) -> Result<f64, FxError> {
        let rate = self.usd_per(currency).await?;
        Ok(amount * rate)
    }
}

/// In-memory rate table — used by tests and as a fallback for
/// disconnected operation. Production code wires the SupabaseFx
/// provider (PR #6) which reads from the `exchange_rates` table.
///
/// USD is always implicitly present at 1.0; explicit entries
/// override.
pub struct StaticFxRates {
    rates: HashMap<String, f64>,
}

impl StaticFxRates {
    pub fn new() -> Self {
        Self {
            rates: HashMap::new(),
        }
    }

    /// Builder-style entry: `StaticFxRates::new().with("EUR", 1.08)`.
    pub fn with(mut self, code: impl Into<String>, usd_per_unit: f64) -> Self {
        self.rates.insert(code.into().to_uppercase(), usd_per_unit);
        self
    }

    pub fn insert(&mut self, code: impl Into<String>, usd_per_unit: f64) {
        self.rates.insert(code.into().to_uppercase(), usd_per_unit);
    }
}

impl Default for StaticFxRates {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FxRateProvider for StaticFxRates {
    async fn usd_per(&self, currency: &str) -> Result<f64, FxError> {
        let code = currency.to_uppercase();
        // USD is implicit — never an error, even on an empty table.
        if code == "USD" {
            return Ok(1.0);
        }
        self.rates
            .get(&code)
            .copied()
            .ok_or_else(|| FxError::UnknownCurrency(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates() -> StaticFxRates {
        StaticFxRates::new()
            .with("EUR", 1.08)
            .with("CNY", 0.14)
            .with("IDR", 0.000064)
    }

    #[tokio::test]
    async fn usd_is_unity_even_when_table_is_empty() {
        // No "USD" entry was added — the provider must still return
        // 1.0. Without this, every quote arriving in USD would
        // trip an `UnknownCurrency` error.
        let r = StaticFxRates::new();
        assert!((r.usd_per("USD").await.unwrap() - 1.0).abs() < 1e-12);
    }

    #[tokio::test]
    async fn known_currency_returns_configured_rate() {
        let r = rates();
        assert!((r.usd_per("EUR").await.unwrap() - 1.08).abs() < 1e-9);
        assert!((r.usd_per("CNY").await.unwrap() - 0.14).abs() < 1e-9);
    }

    #[tokio::test]
    async fn lookup_is_case_insensitive() {
        // ISO 4217 codes are canonically uppercase, but mixed-case
        // input shouldn't trip a false UnknownCurrency.
        let r = rates();
        assert!(r.usd_per("eur").await.is_ok());
        assert!(r.usd_per("Eur").await.is_ok());
        assert!(r.usd_per("EUR").await.is_ok());
    }

    #[tokio::test]
    async fn unknown_currency_surfaces_typed_error() {
        let r = rates();
        let err = r.usd_per("ZWL").await.unwrap_err();
        assert!(matches!(err, FxError::UnknownCurrency(code) if code == "ZWL"));
    }

    #[tokio::test]
    async fn to_usd_multiplies_amount_by_rate() {
        let r = rates();
        // 100 EUR * 1.08 = 108 USD
        let usd = r.to_usd(100.0, "EUR").await.unwrap();
        assert!((usd - 108.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn to_usd_for_usd_amounts_is_identity() {
        let r = StaticFxRates::new();
        let usd = r.to_usd(42.5, "USD").await.unwrap();
        assert!((usd - 42.5).abs() < 1e-12);
    }

    #[tokio::test]
    async fn unknown_currency_propagates_through_to_usd() {
        // Belt-and-braces: to_usd doesn't swallow the underlying
        // usd_per error and silently return 0.0.
        let r = rates();
        let err = r.to_usd(100.0, "ZWL").await.unwrap_err();
        assert!(matches!(err, FxError::UnknownCurrency(_)));
    }

    #[tokio::test]
    async fn insert_replaces_existing_rate() {
        // FX rates drift; the table must let providers refresh in
        // place rather than forcing a reconstruction.
        let mut r = StaticFxRates::new().with("EUR", 1.08);
        r.insert("EUR", 1.10);
        assert!((r.usd_per("EUR").await.unwrap() - 1.10).abs() < 1e-9);
    }
}
