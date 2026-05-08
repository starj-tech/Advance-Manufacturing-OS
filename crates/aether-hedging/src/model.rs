use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Commodity {
    SteelHrc,      // Hot-rolled coil steel
    Aluminum,      // LME aluminum
    Copper,        // LME copper
    Polypropylene, // PP resin
    Brent,         // Brent crude oil
    NaturalGas,    // Henry Hub
    Lithium,       // Lithium carbonate
    Cobalt,        // Cobalt cathode
}

impl Commodity {
    pub fn ticker(&self) -> &'static str {
        match self {
            Commodity::SteelHrc => "HRC",
            Commodity::Aluminum => "ALI",
            Commodity::Copper => "HG",
            Commodity::Polypropylene => "PP",
            Commodity::Brent => "BZ",
            Commodity::NaturalGas => "NG",
            Commodity::Lithium => "LITH",
            Commodity::Cobalt => "CO",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Commodity::SteelHrc => "USD/MT",
            Commodity::Aluminum | Commodity::Copper | Commodity::Lithium | Commodity::Cobalt => {
                "USD/MT"
            }
            Commodity::Polypropylene => "USD/MT",
            Commodity::Brent => "USD/bbl",
            Commodity::NaturalGas => "USD/MMBtu",
        }
    }
}

/// Maps an internal material SKU to the commodity (or commodities) it is
/// exposed to. A single SKU may have multiple exposures (e.g. an EV
/// battery cell tied to lithium + cobalt + copper).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialExposure {
    pub material_sku: String,
    pub commodity: Commodity,
    /// Fraction of the material's cost driven by this commodity (0..=1).
    pub weight: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickers_are_distinct() {
        let cs = [
            Commodity::SteelHrc,
            Commodity::Aluminum,
            Commodity::Copper,
            Commodity::Polypropylene,
            Commodity::Brent,
            Commodity::NaturalGas,
            Commodity::Lithium,
            Commodity::Cobalt,
        ];
        let mut tickers: Vec<&'static str> = cs.iter().map(|c| c.ticker()).collect();
        tickers.sort_unstable();
        let len_before = tickers.len();
        tickers.dedup();
        assert_eq!(len_before, tickers.len());
    }
}
