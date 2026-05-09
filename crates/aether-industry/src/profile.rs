use crate::capability::{cap, Capability, CapabilityKind};
use crate::industry::Industry;
use serde::Serialize;
use thiserror::Error;

/// Auto-installed module ID (resolved against the `modules` registry by
/// the loader at tenant onboarding).
pub type AutoModuleId = &'static str;

/// Serialize-only — profiles are resolved from the code-defined catalog,
/// not deserialized from JSON. Persisted assignments live in
/// `tenant_industry` as a single industry slug, then we re-resolve.
#[derive(Clone, Debug, Serialize)]
pub struct IndustryProfile {
    pub industry: Industry,
    pub capabilities: Vec<Capability>,
    pub auto_modules: Vec<AutoModuleId>,
    /// Compliance standard slugs (resolved against `aether-compliance`).
    pub default_standards: Vec<&'static str>,
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("unknown industry slug: {0}")]
    UnknownSlug(String),
}

/// Resolve the static profile for a given industry.
///
/// All profiles are defined inline so the assignment is deterministic
/// and can be unit-tested. PR #6 layers per-tenant overrides on top.
pub fn profile_for(industry: Industry) -> IndustryProfile {
    use CapabilityKind::*;

    let (capabilities, auto_modules, default_standards) = match industry {
        Industry::FoodAndBeverage => (
            vec![
                cap!(
                    "expired-date-tracking",
                    Tracking,
                    "Expiry date tracking",
                    "Lot/batch expiry, FEFO picking, recall tracing"
                ),
                cap!(
                    "temperature-sensor-integration",
                    Process,
                    "Temperature sensor integration",
                    "Auto-bind OPC-UA / Modbus temperature tags to material lots"
                ),
                cap!(
                    "cold-chain-monitor",
                    Tracking,
                    "Cold chain monitoring",
                    "Continuous temperature & humidity logging with alarms"
                ),
                cap!(
                    "haccp-checks",
                    Quality,
                    "HACCP control points",
                    "Critical control point checklists with corrective actions"
                ),
                cap!(
                    "allergen-control",
                    Compliance,
                    "Allergen control",
                    "Cross-contamination prevention & label verification"
                ),
                cap!(
                    "recall-readiness",
                    Compliance,
                    "Recall readiness",
                    "Trace any unit forwards/backwards in <2 minutes"
                ),
            ],
            vec!["com.aether.cold-chain", "com.aether.haccp"],
            vec!["iso-22000", "haccp", "fssc-22000", "bpom"],
        ),
        Industry::Pharmaceuticals => (
            vec![
                cap!(
                    "gxp-electronic-records",
                    Compliance,
                    "GxP electronic records",
                    "21 CFR Part 11 compliant signatures & audit trails"
                ),
                cap!(
                    "serialization-track-trace",
                    Tracking,
                    "Serialization & track-and-trace",
                    "Per-unit serial numbers, parent-child aggregation, DSCSA reporting"
                ),
                cap!(
                    "batch-genealogy",
                    Tracking,
                    "Batch genealogy",
                    "End-to-end forward & backward traceability"
                ),
                cap!(
                    "environmental-monitoring",
                    Quality,
                    "Environmental monitoring",
                    "Cleanroom particle, viable, and pressure differentials"
                ),
                cap!(
                    "deviation-capa",
                    Quality,
                    "Deviation & CAPA",
                    "Closed-loop corrective and preventive action workflows"
                ),
            ],
            vec!["com.aether.gxp-records", "com.aether.batch-genealogy"],
            vec!["iso-13485", "iso-9001", "fda-21-cfr-11", "gmp", "bpom"],
        ),
        Industry::Automotive => (
            vec![
                cap!(
                    "parts-serial-tracking",
                    Tracking,
                    "Parts serial tracking",
                    "Per-component VIN linkage, supplier→assembly traceability"
                ),
                cap!(
                    "precision-calibration",
                    Quality,
                    "Precision calibration",
                    "Tool wear, gauge R&R, automatic recalibration triggers"
                ),
                cap!(
                    "ppap-control-plan",
                    Quality,
                    "PPAP & control plan",
                    "Production Part Approval Process documentation pack"
                ),
                cap!(
                    "fmea",
                    Quality,
                    "DFMEA / PFMEA",
                    "Failure mode & effects analysis with risk priority numbers"
                ),
                cap!(
                    "andon-line-stop",
                    Safety,
                    "Andon line-stop",
                    "Operator-initiated line stop tied to interlock"
                ),
            ],
            vec!["com.aether.ppap", "com.aether.fmea"],
            vec!["iatf-16949", "iso-9001", "iso-14001"],
        ),
        Industry::Aerospace => (
            vec![
                cap!(
                    "airworthiness-traceability",
                    Tracking,
                    "Airworthiness traceability",
                    "Lot, heat, and composite ply traceability per ATA spec"
                ),
                cap!(
                    "first-article-inspection",
                    Quality,
                    "First article inspection",
                    "AS9102 form-1/2/3 generation"
                ),
                cap!(
                    "dfaars-flowdown",
                    Compliance,
                    "DFARS flowdown",
                    "Cybersecurity & material origin clauses propagated to suppliers"
                ),
                cap!(
                    "torque-controlled-fastening",
                    Quality,
                    "Torque-controlled fastening",
                    "Tool serial + torque + angle captured per fastener"
                ),
            ],
            vec!["com.aether.fai", "com.aether.airworthiness"],
            vec!["as9100", "as9102", "iso-9001"],
        ),
        Industry::Electronics => (
            vec![
                cap!(
                    "smt-line-yield",
                    Quality,
                    "SMT line yield",
                    "Per-feeder pickup, AOI defect Pareto, reflow profile drift"
                ),
                cap!(
                    "conformal-coat-cure",
                    Process,
                    "Conformal coat cure",
                    "Cure-time/temperature with image proof"
                ),
                cap!(
                    "rohs-reach-flowdown",
                    Compliance,
                    "RoHS / REACH flowdown",
                    "Restricted-substance declarations from suppliers"
                ),
                cap!(
                    "esd-safe-zone",
                    Safety,
                    "ESD-safe zone monitor",
                    "Wrist-strap continuity & bench ESD readings"
                ),
            ],
            vec!["com.aether.smt-yield"],
            vec!["iso-9001", "rohs", "reach", "ipc-a-610"],
        ),
        Industry::ChemicalProcess => (
            vec![
                cap!(
                    "recipe-versioning",
                    Process,
                    "Recipe versioning (S88)",
                    "ISA-S88 master recipe → control recipe → batch reports"
                ),
                cap!(
                    "reactive-hazard-control",
                    Safety,
                    "Reactive hazard control",
                    "Compatibility matrix, segregation, exotherm alarms"
                ),
                cap!(
                    "ghs-labeling",
                    Compliance,
                    "GHS labeling",
                    "Hazard pictogram + signal-word generation"
                ),
                cap!(
                    "emissions-tracking",
                    Sustainability,
                    "Emissions tracking",
                    "VOC, GHG, and effluent rolling totals"
                ),
            ],
            vec!["com.aether.recipes-s88"],
            vec!["iso-9001", "iso-14001", "iso-45001", "reach"],
        ),
        Industry::TextileAndApparel => (
            vec![
                cap!(
                    "dye-lot-color-control",
                    Quality,
                    "Dye lot color control",
                    "ΔE measurement vs reference, lot-level acceptance"
                ),
                cap!(
                    "size-matrix-allocation",
                    Process,
                    "Size matrix allocation",
                    "Cut-marker optimization, size-balanced bundling"
                ),
                cap!(
                    "compliance-bsci",
                    Compliance,
                    "BSCI / WRAP compliance",
                    "Social audit evidence pack"
                ),
            ],
            vec!["com.aether.dye-lots"],
            vec!["iso-9001", "iso-14001", "oeko-tex"],
        ),
        Industry::Plastics => (
            vec![
                cap!(
                    "injection-cycle-monitor",
                    Process,
                    "Injection cycle monitor",
                    "Cycle time, cushion, peak pressure SPC per cavity"
                ),
                cap!(
                    "regrind-ratio-tracking",
                    Sustainability,
                    "Regrind ratio",
                    "Virgin/regrind blend per shot for spec adherence"
                ),
                cap!(
                    "rohs-reach-flowdown",
                    Compliance,
                    "RoHS / REACH flowdown",
                    "Restricted-substance declarations from resin suppliers"
                ),
            ],
            vec!["com.aether.injection-spc"],
            vec!["iso-9001", "rohs", "reach"],
        ),
        Industry::MetalFabrication => (
            vec![
                cap!(
                    "welder-qualification",
                    Safety,
                    "Welder qualification",
                    "WPS / PQR / WPQ matched per joint"
                ),
                cap!(
                    "ndt-records",
                    Quality,
                    "NDT records",
                    "Radiographic / ultrasonic / dye-pen results per weld"
                ),
                cap!(
                    "heat-treatment-curve",
                    Process,
                    "Heat treatment curves",
                    "Furnace ramp/soak/quench traceable to part"
                ),
            ],
            vec!["com.aether.weld-qualification"],
            vec!["iso-9001", "iso-3834", "asme-section-ix"],
        ),
        Industry::WoodAndPaper => (
            vec![
                cap!(
                    "moisture-monitor",
                    Process,
                    "Moisture monitor",
                    "In-line moisture meter + drying curve"
                ),
                cap!(
                    "fsc-chain-of-custody",
                    Compliance,
                    "FSC chain of custody",
                    "Forest stewardship lot tracking from log to product"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001", "fsc"],
        ),
        Industry::Cement => (
            vec![
                cap!(
                    "clinker-quality",
                    Quality,
                    "Clinker quality",
                    "Free-lime, LSF, SR, AR continuous monitoring"
                ),
                cap!(
                    "kiln-thermal-balance",
                    Sustainability,
                    "Kiln thermal balance",
                    "Heat-rate per ton, CO2 intensity"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001", "iso-50001"],
        ),
        Industry::Mining => (
            vec![
                cap!(
                    "ore-grade-tracking",
                    Tracking,
                    "Ore grade tracking",
                    "Per-shovel grade & destination routing"
                ),
                cap!(
                    "blast-pattern-record",
                    Safety,
                    "Blast pattern record",
                    "Hole layout, charge weight, exclusion-zone proof"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001", "iso-45001"],
        ),
        Industry::OilAndGas => (
            vec![
                cap!(
                    "pipeline-integrity",
                    Safety,
                    "Pipeline integrity",
                    "Pig run, corrosion-coupon results, MAOP"
                ),
                cap!(
                    "flare-emissions",
                    Sustainability,
                    "Flare emissions",
                    "Continuous flare composition + flow"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001", "iso-45001", "api-q1"],
        ),
        Industry::Energy => (
            vec![
                cap!(
                    "scada-aggregation",
                    Process,
                    "SCADA aggregation",
                    "Tag normalization across substations / plants"
                ),
                cap!(
                    "renewables-curtailment",
                    Sustainability,
                    "Renewables curtailment",
                    "Forecast vs actual, curtailment cost accounting"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001", "iso-50001"],
        ),
        Industry::MedicalDevices => (
            vec![
                cap!(
                    "udi-marking",
                    Tracking,
                    "UDI marking",
                    "Unique Device Identifier laser/inkjet verification"
                ),
                cap!(
                    "design-history-file",
                    Compliance,
                    "Design History File",
                    "Linked DHF / DMR / DHR per device family"
                ),
                cap!(
                    "biocompat-evidence",
                    Quality,
                    "Biocompatibility evidence",
                    "Per-material ISO 10993 packet"
                ),
            ],
            vec!["com.aether.udi-marking"],
            vec!["iso-13485", "fda-21-cfr-820", "ce-mdr"],
        ),
        Industry::Cosmetics => (
            vec![
                cap!(
                    "formula-protection",
                    Compliance,
                    "Formula protection",
                    "Recipe encrypted client-side; version locked"
                ),
                cap!(
                    "microbiological-testing",
                    Quality,
                    "Microbiological testing",
                    "Per-lot challenge & preservative efficacy"
                ),
            ],
            vec![],
            vec!["iso-22716", "iso-9001", "bpom"],
        ),
        Industry::Furniture => (
            vec![
                cap!(
                    "dimension-quality",
                    Quality,
                    "Dimension quality",
                    "CMM-fed dimensional check vs CAD"
                ),
                cap!(
                    "voc-emissions",
                    Sustainability,
                    "VOC emissions",
                    "Finish VOC tracking against E1/CARB"
                ),
            ],
            vec![],
            vec!["iso-9001", "fsc", "carb-atcm-93120"],
        ),
        Industry::GlassAndCeramics => (
            vec![
                cap!(
                    "kiln-firing-curve",
                    Process,
                    "Kiln firing curve",
                    "Ramp/soak/cool with thermal-shock alarms"
                ),
                cap!(
                    "optical-inspection",
                    Quality,
                    "Optical inspection",
                    "Inclusion / bubble / surface defect via line-scan"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-14001"],
        ),
        Industry::Rubber => (
            vec![
                cap!(
                    "mooney-viscosity",
                    Quality,
                    "Mooney viscosity",
                    "Continuous viscosity SPC vs spec"
                ),
                cap!(
                    "cure-curve",
                    Process,
                    "Cure curve",
                    "Time-to-90 (t90) with mold-temperature trace"
                ),
            ],
            vec![],
            vec!["iso-9001"],
        ),
        Industry::PrintingAndPackaging => (
            vec![
                cap!(
                    "color-spectro-control",
                    Quality,
                    "Color spectro control",
                    "ΔE alarming with auto-stop vs reference target"
                ),
                cap!(
                    "food-contact-compliance",
                    Compliance,
                    "Food-contact compliance",
                    "Migration tests + compositional declarations"
                ),
            ],
            vec![],
            vec!["iso-9001", "iso-22000", "iso-15378"],
        ),
        Industry::BatteryAndRenewables => (
            vec![
                cap!(
                    "cell-formation-tracking",
                    Tracking,
                    "Cell formation tracking",
                    "Per-cell capacity, IR, & coulombic efficiency"
                ),
                cap!(
                    "thermal-runaway-watch",
                    Safety,
                    "Thermal runaway watch",
                    "Cell-bank thermal anomaly detection"
                ),
                cap!(
                    "battery-passport",
                    Compliance,
                    "EU Battery Passport",
                    "Material origin, recycled content, carbon footprint per cell"
                ),
            ],
            vec!["com.aether.battery-passport"],
            vec!["iso-9001", "iso-14001", "eu-battery-regulation", "un-38-3"],
        ),
    };

    IndustryProfile {
        industry,
        capabilities,
        auto_modules,
        default_standards,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::industry::INDUSTRIES;

    #[test]
    fn every_industry_has_at_least_one_capability() {
        for i in INDUSTRIES {
            let p = profile_for(i);
            assert!(!p.capabilities.is_empty(), "{:?} has no capabilities", i);
        }
    }

    #[test]
    fn food_industry_has_expiry_tracking() {
        let p = profile_for(Industry::FoodAndBeverage);
        assert!(p
            .capabilities
            .iter()
            .any(|c| c.id == "expired-date-tracking"));
    }

    #[test]
    fn food_industry_has_temperature_sensor_integration() {
        let p = profile_for(Industry::FoodAndBeverage);
        assert!(p
            .capabilities
            .iter()
            .any(|c| c.id == "temperature-sensor-integration"));
    }

    #[test]
    fn automotive_has_serial_and_calibration() {
        let p = profile_for(Industry::Automotive);
        assert!(p
            .capabilities
            .iter()
            .any(|c| c.id == "parts-serial-tracking"));
        assert!(p
            .capabilities
            .iter()
            .any(|c| c.id == "precision-calibration"));
    }

    #[test]
    fn pharma_has_gxp_records() {
        let p = profile_for(Industry::Pharmaceuticals);
        assert!(p
            .capabilities
            .iter()
            .any(|c| c.id == "gxp-electronic-records"));
    }

    #[test]
    fn capability_ids_within_a_profile_are_unique() {
        for i in INDUSTRIES {
            let p = profile_for(i);
            let mut ids: Vec<_> = p.capabilities.iter().map(|c| c.id).collect();
            ids.sort_unstable();
            let len_before = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), len_before, "duplicate capability id in {:?}", i);
        }
    }
}
