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

    /// Reverse of [`ticker`] — used by the HTTP feed when decoding
    /// observations whose only commodity identifier is the ticker
    /// string. Returns `None` for unrecognized tickers (the caller
    /// usually skips that row rather than failing the whole batch).
    pub fn from_ticker(s: &str) -> Option<Self> {
        match s {
            "HRC" => Some(Commodity::SteelHrc),
            "ALI" => Some(Commodity::Aluminum),
            "HG" => Some(Commodity::Copper),
            "PP" => Some(Commodity::Polypropylene),
            "BZ" => Some(Commodity::Brent),
            "NG" => Some(Commodity::NaturalGas),
            "LITH" => Some(Commodity::Lithium),
            "CO" => Some(Commodity::Cobalt),
            _ => None,
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

    /// The complete catalog. Centralised so the test below and any
    /// future enum-exhaustive test consult the same list.
    const ALL_COMMODITIES: &[Commodity] = &[
        Commodity::SteelHrc,
        Commodity::Aluminum,
        Commodity::Copper,
        Commodity::Polypropylene,
        Commodity::Brent,
        Commodity::NaturalGas,
        Commodity::Lithium,
        Commodity::Cobalt,
    ];

    #[test]
    fn tickers_are_distinct() {
        let mut tickers: Vec<&'static str> = ALL_COMMODITIES.iter().map(|c| c.ticker()).collect();
        tickers.sort_unstable();
        let len_before = tickers.len();
        tickers.dedup();
        assert_eq!(len_before, tickers.len());
    }

    #[test]
    fn ticker_round_trips_through_from_ticker_for_every_commodity() {
        // Property: ticker() and from_ticker() are mutual inverses for
        // every variant. If anyone adds a new Commodity, this test
        // breaks until they wire both directions.
        for c in ALL_COMMODITIES {
            let t = c.ticker();
            assert_eq!(
                Commodity::from_ticker(t),
                Some(*c),
                "round-trip broken for {c:?} (ticker={t})"
            );
        }
    }

    #[test]
    fn from_ticker_rejects_unknown_strings() {
        // Unknown tickers must not silently map onto a default
        // Commodity — that's how an "ALU" typo would land copper
        // prices into the aluminum bucket.
        assert_eq!(Commodity::from_ticker(""), None);
        assert_eq!(Commodity::from_ticker("ALU"), None);
        assert_eq!(Commodity::from_ticker("hrc"), None, "case-sensitive");
    }
}
