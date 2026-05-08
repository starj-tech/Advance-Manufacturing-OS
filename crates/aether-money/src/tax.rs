use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaxScope {
    /// VAT-equivalent (Indonesia PPN, EU VAT, UK VAT, AU GST, ...).
    ValueAdded,
    /// US-style state/county sales tax.
    Sales,
    /// Service/withholding (PPh in Indonesia, etc).
    Withholding,
    /// Excise (fuel, tobacco, alcohol).
    Excise,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaxJurisdiction {
    /// ISO-3166 alpha-2 country code (e.g. "ID", "US", "DE").
    pub country: String,
    /// Optional sub-region (state, province, regency).
    pub subdivision: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaxRule {
    pub jurisdiction: TaxJurisdiction,
    pub scope: TaxScope,
    /// Rate in basis points (1.00% = 100 bps; 11.00% PPN = 1100 bps).
    pub rate_bps: i32,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    /// Display name shown on invoices ("PPN 11%", "VAT 20%", "GST 10%").
    pub display: String,
}

impl TaxRule {
    pub fn is_active_at(&self, instant: DateTime<Utc>) -> bool {
        if instant < self.valid_from {
            return false;
        }
        if let Some(end) = self.valid_until {
            if instant >= end {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn rule_active_within_window() {
        let now = Utc::now();
        let r = TaxRule {
            jurisdiction: TaxJurisdiction {
                country: "ID".into(),
                subdivision: None,
            },
            scope: TaxScope::ValueAdded,
            rate_bps: 1100,
            valid_from: now - Duration::days(30),
            valid_until: Some(now + Duration::days(30)),
            display: "PPN 11%".into(),
        };
        assert!(r.is_active_at(now));
        assert!(!r.is_active_at(now - Duration::days(60)));
        assert!(!r.is_active_at(now + Duration::days(60)));
    }
}
