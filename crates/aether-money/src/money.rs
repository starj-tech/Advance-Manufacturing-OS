use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ISO-4217 currency code with the number of minor-unit decimals
/// required to format amounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Currency {
    pub code: [u8; 3],
    pub minor_units: u8,
}

impl Currency {
    pub const fn new(code: &'static str, minor_units: u8) -> Self {
        let b = code.as_bytes();
        Self {
            code: [b[0], b[1], b[2]],
            minor_units,
        }
    }

    pub fn code_str(&self) -> &str {
        std::str::from_utf8(&self.code).unwrap_or("???")
    }
}

pub const USD: Currency = Currency::new("USD", 2);
pub const EUR: Currency = Currency::new("EUR", 2);
pub const IDR: Currency = Currency::new("IDR", 0);
pub const JPY: Currency = Currency::new("JPY", 0);
pub const SGD: Currency = Currency::new("SGD", 2);
pub const CNY: Currency = Currency::new("CNY", 2);
pub const GBP: Currency = Currency::new("GBP", 2);
pub const AUD: Currency = Currency::new("AUD", 2);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Money {
    /// Amount in minor units (e.g. cents for USD, sen for IDR... but
    /// IDR has 0 minor digits so this equals rupiah).
    pub minor: i64,
    pub currency: Currency,
}

#[derive(Debug, Error)]
pub enum MoneyError {
    #[error("currency mismatch: {0} != {1}")]
    CurrencyMismatch(String, String),
    #[error("overflow")]
    Overflow,
}

impl Money {
    pub fn new(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }

    /// Checked addition with currency mismatch detection. We deliberately
    /// don't `impl Add` — silently panicking on currency mismatch in
    /// financial code would be worse than the ergonomic cost of a method.
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.code_str().into(),
                other.currency.code_str().into(),
            ));
        }
        let minor = self
            .minor
            .checked_add(other.minor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self {
            minor,
            currency: self.currency,
        })
    }

    pub fn mul_bps(self, bps: i32) -> Result<Self, MoneyError> {
        // amount * bps / 10000, with checked arithmetic.
        let prod = self
            .minor
            .checked_mul(bps as i64)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self {
            minor: prod / 10_000,
            currency: self.currency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_same_currency_works() {
        let a = Money::new(100, USD);
        let b = Money::new(50, USD);
        assert_eq!(a.checked_add(b).unwrap(), Money::new(150, USD));
    }

    #[test]
    fn add_different_currency_fails() {
        let a = Money::new(100, USD);
        let b = Money::new(50, EUR);
        assert!(a.checked_add(b).is_err());
    }

    #[test]
    fn idr_uses_zero_minor_digits() {
        assert_eq!(IDR.minor_units, 0);
    }

    #[test]
    fn mul_bps_for_tax() {
        // 10,000 cents (USD $100) at 7.25% sales tax = 725 cents.
        let m = Money::new(10_000, USD).mul_bps(725).unwrap();
        assert_eq!(m, Money::new(725, USD));
    }
}
