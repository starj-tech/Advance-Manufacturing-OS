use serde::{Deserialize, Serialize};

/// 21 industry verticals supported by AETHER-OS at launch.
///
/// Backing the discriminator with `kebab-case` makes the wire format
/// stable across language boundaries and JSON storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Industry {
    FoodAndBeverage,
    Pharmaceuticals,
    Automotive,
    Aerospace,
    Electronics,
    ChemicalProcess,
    TextileAndApparel,
    Plastics,
    MetalFabrication,
    WoodAndPaper,
    Cement,
    Mining,
    OilAndGas,
    Energy,
    MedicalDevices,
    Cosmetics,
    Furniture,
    GlassAndCeramics,
    Rubber,
    PrintingAndPackaging,
    BatteryAndRenewables,
}

impl Industry {
    pub fn slug(&self) -> &'static str {
        match self {
            Industry::FoodAndBeverage => "food-and-beverage",
            Industry::Pharmaceuticals => "pharmaceuticals",
            Industry::Automotive => "automotive",
            Industry::Aerospace => "aerospace",
            Industry::Electronics => "electronics",
            Industry::ChemicalProcess => "chemical-process",
            Industry::TextileAndApparel => "textile-and-apparel",
            Industry::Plastics => "plastics",
            Industry::MetalFabrication => "metal-fabrication",
            Industry::WoodAndPaper => "wood-and-paper",
            Industry::Cement => "cement",
            Industry::Mining => "mining",
            Industry::OilAndGas => "oil-and-gas",
            Industry::Energy => "energy",
            Industry::MedicalDevices => "medical-devices",
            Industry::Cosmetics => "cosmetics",
            Industry::Furniture => "furniture",
            Industry::GlassAndCeramics => "glass-and-ceramics",
            Industry::Rubber => "rubber",
            Industry::PrintingAndPackaging => "printing-and-packaging",
            Industry::BatteryAndRenewables => "battery-and-renewables",
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Industry::FoodAndBeverage => "Food & Beverage",
            Industry::Pharmaceuticals => "Pharmaceuticals",
            Industry::Automotive => "Automotive",
            Industry::Aerospace => "Aerospace",
            Industry::Electronics => "Electronics",
            Industry::ChemicalProcess => "Chemical (Process)",
            Industry::TextileAndApparel => "Textile & Apparel",
            Industry::Plastics => "Plastics",
            Industry::MetalFabrication => "Metal Fabrication",
            Industry::WoodAndPaper => "Wood & Paper",
            Industry::Cement => "Cement & Construction",
            Industry::Mining => "Mining & Quarrying",
            Industry::OilAndGas => "Oil & Gas",
            Industry::Energy => "Energy & Utilities",
            Industry::MedicalDevices => "Medical Devices",
            Industry::Cosmetics => "Cosmetics & Personal Care",
            Industry::Furniture => "Furniture",
            Industry::GlassAndCeramics => "Glass & Ceramics",
            Industry::Rubber => "Rubber",
            Industry::PrintingAndPackaging => "Printing & Packaging",
            Industry::BatteryAndRenewables => "Battery & Renewables",
        }
    }
}

/// Stable iteration order — used by the UI's industry picker.
pub const INDUSTRIES: [Industry; 21] = [
    Industry::FoodAndBeverage,
    Industry::Pharmaceuticals,
    Industry::Automotive,
    Industry::Aerospace,
    Industry::Electronics,
    Industry::ChemicalProcess,
    Industry::TextileAndApparel,
    Industry::Plastics,
    Industry::MetalFabrication,
    Industry::WoodAndPaper,
    Industry::Cement,
    Industry::Mining,
    Industry::OilAndGas,
    Industry::Energy,
    Industry::MedicalDevices,
    Industry::Cosmetics,
    Industry::Furniture,
    Industry::GlassAndCeramics,
    Industry::Rubber,
    Industry::PrintingAndPackaging,
    Industry::BatteryAndRenewables,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn slugs_are_unique() {
        let mut seen: HashSet<&'static str> = HashSet::new();
        for i in INDUSTRIES {
            assert!(seen.insert(i.slug()), "duplicate slug for {:?}", i);
        }
    }

    #[test]
    fn industries_array_has_all_variants() {
        // If a new variant is added without updating INDUSTRIES, this
        // test won't catch it directly — but the next two assertions
        // ensure the listing matches reality at the size level.
        assert_eq!(INDUSTRIES.len(), 21);
        let unique: HashSet<_> = INDUSTRIES.iter().collect();
        assert_eq!(unique.len(), 21);
    }
}
