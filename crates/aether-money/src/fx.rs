use crate::money::{Currency, Money, MoneyError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// FX rate snapshot. `as_of` is required so historical transactions can
/// be reproduced exactly, even when the rate later changes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FxRate {
    pub from: Currency,
    pub to: Currency,
    /// Multiplier applied to `from.minor` after scaling for minor-unit
    /// difference. Stored as basis points × 1e4 (i.e. value * 1e8) so we
    /// can serialize an integer instead of a float.
    pub bp1e4: i64,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("money: {0}")]
    Money(#[from] MoneyError),
    #[error("rate currency mismatch")]
    CurrencyMismatch,
    #[error("overflow")]
    Overflow,
}

pub fn convert(amount: Money, rate: FxRate) -> Result<Money, ConvertError> {
    if amount.currency != rate.from {
        return Err(ConvertError::CurrencyMismatch);
    }

    // Adjust for minor-unit difference between currencies.
    let minor_diff = rate.to.minor_units as i32 - rate.from.minor_units as i32;
    let scale: i64 = match minor_diff.cmp(&0) {
        std::cmp::Ordering::Equal => 1,
        std::cmp::Ordering::Greater => 10_i64.pow(minor_diff as u32),
        std::cmp::Ordering::Less => -(10_i64.pow((-minor_diff) as u32)),
    };

    let cross = amount
        .minor
        .checked_mul(rate.bp1e4)
        .ok_or(ConvertError::Overflow)?;
    let mut result = cross / 100_000_000_i64;

    if scale > 1 {
        result = result.checked_mul(scale).ok_or(ConvertError::Overflow)?;
    } else if scale < -1 {
        result /= -scale;
    }

    Ok(Money::new(result, rate.to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::{IDR, USD};

    #[test]
    fn usd_to_idr_high_minor_diff() {
        // $1 USD → IDR 16,000 (assume rate)
        // bp1e4 = 16_000 * 1e8 = 1.6e12
        let rate = FxRate {
            from: USD,
            to: IDR,
            bp1e4: 1_600_000_000_000,
            as_of: Utc::now(),
        };
        let one_usd = Money::new(100, USD); // 100 cents = $1
        let in_idr = convert(one_usd, rate).unwrap();
        assert_eq!(in_idr.currency, IDR);
        // (100 * 1.6e12) / 1e8 = 1_600_000; minor_diff = 0-2 = -2 → /100
        assert_eq!(in_idr.minor, 16_000);
    }

    #[test]
    fn rejects_mismatched_source() {
        let rate = FxRate {
            from: USD,
            to: IDR,
            bp1e4: 1_600_000_000_000,
            as_of: Utc::now(),
        };
        let one_eur = Money::new(100, crate::money::EUR);
        assert!(matches!(
            convert(one_eur, rate),
            Err(ConvertError::CurrencyMismatch)
        ));
    }
}
