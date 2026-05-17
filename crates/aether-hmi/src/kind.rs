//! `HmiKind` — discriminator across the four HMI surface
//! classes.
//!
//! ## Why a typed enum
//! Different surfaces have different gesture surfaces and
//! different rendering affordances. A pendant has a 4-line LCD
//! and a few hardware buttons; smart glasses have AR overlay +
//! voice + gaze. A typed kind lets the binding wizard branch
//! cleanly (route Voice events only to surfaces that can emit
//! them, render Critical announcements differently per surface)
//! without proliferating trait variants. Adding a kind is a
//! deliberate workspace-wide change.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiKind {
    /// Fixed-mount factory HMI (Siemens Comfort Panel, AB
    /// PanelView, Schneider Magelis). Touchscreen + optional
    /// hardware buttons; no voice.
    Touchscreen,
    /// Operator-carried Zebra ET6x / Honeywell EDA61K. Camera,
    /// barcode trigger button (cross-wired to V11 Scanner),
    /// occasional voice.
    RuggedTablet,
    /// RealWear HMT-1 / Vuzix Blade. AR overlay surface +
    /// always-on voice + gaze-dwell selection. No touch.
    SmartGlasses,
    /// Machine-bound handheld pendant (FANUC iPendant, KUKA
    /// smartPAD). Small screen, mostly hardware buttons +
    /// jog wheel.
    Pendant,
}

impl HmiKind {
    /// Kebab-case slug for the audit ledger and binding TOML.
    /// Renaming a variant is a schema migration.
    pub fn slug(&self) -> &'static str {
        match self {
            HmiKind::Touchscreen => "touchscreen",
            HmiKind::RuggedTablet => "rugged-tablet",
            HmiKind::SmartGlasses => "smart-glasses",
            HmiKind::Pendant => "pendant",
        }
    }

    /// Whether this surface can emit voice utterances. The
    /// pipeline uses this to route voice handlers to compatible
    /// surfaces only, so a workflow that asks "say YES or NO"
    /// doesn't get queued on a fixed touchscreen.
    pub fn supports_voice(&self) -> bool {
        matches!(self, HmiKind::SmartGlasses | HmiKind::RuggedTablet)
    }

    /// Whether this surface can emit gaze-dwell events. Smart
    /// glasses only at the moment; some industrial AR helmets
    /// will join later.
    pub fn supports_gaze(&self) -> bool {
        matches!(self, HmiKind::SmartGlasses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let kinds = [
            HmiKind::Touchscreen,
            HmiKind::RuggedTablet,
            HmiKind::SmartGlasses,
            HmiKind::Pendant,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|k| k.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn voice_capability_distinguishes_surfaces_correctly() {
        assert!(!HmiKind::Touchscreen.supports_voice());
        assert!(HmiKind::RuggedTablet.supports_voice());
        assert!(HmiKind::SmartGlasses.supports_voice());
        assert!(!HmiKind::Pendant.supports_voice());
    }

    #[test]
    fn gaze_capability_is_smart_glasses_only() {
        assert!(!HmiKind::Touchscreen.supports_gaze());
        assert!(!HmiKind::RuggedTablet.supports_gaze());
        assert!(HmiKind::SmartGlasses.supports_gaze());
        assert!(!HmiKind::Pendant.supports_gaze());
    }

    #[test]
    fn rugged_tablet_slug_uses_dash() {
        let s = serde_json::to_string(&HmiKind::RuggedTablet).unwrap();
        assert_eq!(s, "\"rugged-tablet\"");
        let back: HmiKind = serde_json::from_str("\"rugged-tablet\"").unwrap();
        assert_eq!(back, HmiKind::RuggedTablet);
    }
}
