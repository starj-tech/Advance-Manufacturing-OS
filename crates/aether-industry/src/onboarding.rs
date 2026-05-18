//! Tenant onboarding input validation.
//!
//! ## What this is
//! The bottom layer of the first-boot wizard flow. Takes raw
//! operator input (slug, display name, region, industry) and
//! returns either a validated `OnboardingInput` ready to be
//! handed to the tenant-creation RPC, or a typed
//! `OnboardingError` with a message the wizard can render
//! directly next to the offending field.
//!
//! ## What this is NOT
//! Persistence. This module validates SHAPES — slug grammar,
//! region whitelist, name length — but doesn't talk to the
//! database. A future tenant-creation RPC would call
//! `validate()` first, then write to the `tenants` table.
//!
//! ## Why slug rules are strict
//! Tenant slugs appear in URLs (`aether-os.com/<slug>/...`),
//! in keystore handles (`tenant-<slug>/master`), and in audit
//! logs. Allowing arbitrary unicode would invite homograph
//! attacks (`аpple` vs `apple`); allowing trailing dashes or
//! double-dashes would break URL parsers. The grammar is
//! intentionally narrow: lowercase ASCII letters, digits, and
//! single hyphens between non-hyphen runs.

use crate::industry::Industry;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Minimum slug length. 3 chars catches typos (`a` is too
/// easy to misclick); 3 chars also matches the typical
/// "shortest meaningful name" floor.
pub const MIN_SLUG_LEN: usize = 3;

/// Maximum slug length. 64 chars covers every brand name
/// we've seen in pilots; longer is a bug (someone pasted a
/// URL into the wrong field).
pub const MAX_SLUG_LEN: usize = 64;

/// Maximum display-name length. 128 chars covers
/// internationalized brand names with subtitles; longer
/// truncates oddly in UI.
pub const MAX_NAME_LEN: usize = 128;

/// Region whitelist. Each region is a (data-residency) zone
/// the production Supabase project ships with; adding a region
/// requires server-side capacity, not just a code change.
pub const ALLOWED_REGIONS: &[&str] = &[
    "us-east-1",
    "us-west-2",
    "eu-west-1",
    "eu-central-1",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-northeast-1",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnboardingInput {
    pub slug: String,
    pub display_name: String,
    pub region: String,
    pub industry: Industry,
}

#[derive(Debug, Error)]
pub enum OnboardingError {
    /// Slug too short / too long / wrong characters. The
    /// variant carries the offending value back so the wizard
    /// can highlight the field.
    #[error("slug invalid: {0}")]
    InvalidSlug(String),
    /// Display name empty or too long.
    #[error("display_name invalid: {0}")]
    InvalidDisplayName(String),
    /// Region not in the whitelist. Surface the requested
    /// region in the error so the wizard can render "X is not
    /// available; pick one of …".
    #[error("region {0} not supported; allowed: {}", ALLOWED_REGIONS.join(", "))]
    UnsupportedRegion(String),
}

/// Validate the slug grammar — lowercase ASCII alphanumerics
/// plus single hyphens between non-hyphen runs. Returns the
/// trimmed slug on success or a descriptive error on failure.
pub fn validate_slug(raw: &str) -> Result<String, OnboardingError> {
    let s = raw.trim();
    if s.len() < MIN_SLUG_LEN {
        return Err(OnboardingError::InvalidSlug(format!(
            "must be at least {MIN_SLUG_LEN} chars (got {})",
            s.len()
        )));
    }
    if s.len() > MAX_SLUG_LEN {
        return Err(OnboardingError::InvalidSlug(format!(
            "must be at most {MAX_SLUG_LEN} chars (got {})",
            s.len()
        )));
    }
    if s.starts_with('-') || s.ends_with('-') {
        return Err(OnboardingError::InvalidSlug(
            "must not start or end with '-'".into(),
        ));
    }
    if s.contains("--") {
        return Err(OnboardingError::InvalidSlug(
            "must not contain '--' (use single hyphens)".into(),
        ));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(OnboardingError::InvalidSlug(
            "only lowercase a-z, 0-9, and '-' allowed".into(),
        ));
    }
    Ok(s.to_string())
}

/// Validate the display name — non-empty trim, length capped.
pub fn validate_display_name(raw: &str) -> Result<String, OnboardingError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(OnboardingError::InvalidDisplayName(
            "must not be empty".into(),
        ));
    }
    if s.len() > MAX_NAME_LEN {
        return Err(OnboardingError::InvalidDisplayName(format!(
            "must be at most {MAX_NAME_LEN} chars (got {})",
            s.len()
        )));
    }
    Ok(s.to_string())
}

/// Validate the region against the whitelist.
pub fn validate_region(raw: &str) -> Result<String, OnboardingError> {
    let s = raw.trim();
    if ALLOWED_REGIONS.contains(&s) {
        Ok(s.to_string())
    } else {
        Err(OnboardingError::UnsupportedRegion(s.to_string()))
    }
}

/// Full-input validator. Runs the three field validators in
/// order; returns the first error. Pin: this is the only
/// path into a valid `OnboardingInput` — the struct is
/// constructible directly from caller code, but a valid one
/// always goes through here.
pub fn validate(
    slug: &str,
    display_name: &str,
    region: &str,
    industry: Industry,
) -> Result<OnboardingInput, OnboardingError> {
    Ok(OnboardingInput {
        slug: validate_slug(slug)?,
        display_name: validate_display_name(display_name)?,
        region: validate_region(region)?,
        industry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_validates_with_all_fields() {
        let input = validate(
            "acme-foods",
            "Acme Foods Tbk.",
            "ap-southeast-1",
            Industry::FoodAndBeverage,
        )
        .unwrap();
        assert_eq!(input.slug, "acme-foods");
        assert_eq!(input.display_name, "Acme Foods Tbk.");
        assert_eq!(input.region, "ap-southeast-1");
        assert_eq!(input.industry, Industry::FoodAndBeverage);
    }

    #[test]
    fn slug_trimmed_of_leading_trailing_whitespace() {
        let input = validate("  acme  ", "Acme", "us-east-1", Industry::Plastics).unwrap();
        assert_eq!(input.slug, "acme");
    }

    #[test]
    fn slug_too_short_rejected() {
        let err = validate_slug("ab").unwrap_err();
        assert!(matches!(err, OnboardingError::InvalidSlug(_)));
    }

    #[test]
    fn slug_too_long_rejected() {
        let huge = "a".repeat(MAX_SLUG_LEN + 1);
        let err = validate_slug(&huge).unwrap_err();
        assert!(matches!(err, OnboardingError::InvalidSlug(_)));
    }

    #[test]
    fn slug_at_min_and_max_boundaries_accepted_inclusive() {
        validate_slug(&"a".repeat(MIN_SLUG_LEN)).unwrap();
        validate_slug(&"a".repeat(MAX_SLUG_LEN)).unwrap();
    }

    #[test]
    fn slug_with_leading_or_trailing_hyphen_rejected() {
        assert!(matches!(
            validate_slug("-acme"),
            Err(OnboardingError::InvalidSlug(_))
        ));
        assert!(matches!(
            validate_slug("acme-"),
            Err(OnboardingError::InvalidSlug(_))
        ));
    }

    #[test]
    fn slug_with_double_hyphen_rejected() {
        assert!(matches!(
            validate_slug("acme--foods"),
            Err(OnboardingError::InvalidSlug(_))
        ));
    }

    #[test]
    fn slug_uppercase_rejected() {
        // Homograph + case-insensitive URL gotcha.
        assert!(matches!(
            validate_slug("Acme"),
            Err(OnboardingError::InvalidSlug(_))
        ));
    }

    #[test]
    fn slug_non_ascii_rejected() {
        // The kind of homograph attack the strict grammar
        // exists to prevent: "аpple" with a cyrillic 'а'.
        assert!(matches!(
            validate_slug("аpple"),
            Err(OnboardingError::InvalidSlug(_))
        ));
    }

    #[test]
    fn slug_underscore_rejected() {
        // URL convention favors hyphens; pin the underscore
        // rejection so the spec doesn't drift toward "either
        // works."
        assert!(matches!(
            validate_slug("acme_foods"),
            Err(OnboardingError::InvalidSlug(_))
        ));
    }

    #[test]
    fn slug_with_digits_accepted() {
        validate_slug("acme-2026").unwrap();
        validate_slug("12345-foo").unwrap();
    }

    #[test]
    fn display_name_empty_rejected() {
        assert!(matches!(
            validate_display_name(""),
            Err(OnboardingError::InvalidDisplayName(_))
        ));
        assert!(matches!(
            validate_display_name("   "),
            Err(OnboardingError::InvalidDisplayName(_))
        ));
    }

    #[test]
    fn display_name_too_long_rejected() {
        let huge = "a".repeat(MAX_NAME_LEN + 1);
        assert!(matches!(
            validate_display_name(&huge),
            Err(OnboardingError::InvalidDisplayName(_))
        ));
    }

    #[test]
    fn display_name_unicode_accepted() {
        // Brand names use emoji, accents, kanji — accept the
        // full unicode space here, only the slug needs to
        // stay ASCII.
        let n = validate_display_name("Açai Café 株式会社 🌱").unwrap();
        assert_eq!(n, "Açai Café 株式会社 🌱");
    }

    #[test]
    fn region_outside_whitelist_rejected_with_suggestion() {
        let err = validate_region("antarctica-1").unwrap_err();
        match err {
            OnboardingError::UnsupportedRegion(r) => {
                assert_eq!(r, "antarctica-1");
            }
            other => panic!("expected UnsupportedRegion, got {other:?}"),
        }
    }

    #[test]
    fn every_allowed_region_validates() {
        // Pin every entry in the whitelist actually validates
        // — catches the typo-in-the-allow-list bug.
        for r in ALLOWED_REGIONS {
            assert_eq!(&validate_region(r).unwrap(), *r);
        }
    }

    #[test]
    fn validate_returns_first_error_in_field_order() {
        // Pin field-order: slug > display_name > region. A
        // wizard that surfaces "fix this one" wants the
        // topmost field error first.
        let err = validate(
            "BAD-slug",   // invalid slug
            "",           // invalid name too — but slug fails first
            "antarctica", // invalid region too
            Industry::Aerospace,
        )
        .unwrap_err();
        assert!(matches!(err, OnboardingError::InvalidSlug(_)));
    }

    #[test]
    fn slug_with_only_digits_accepted() {
        // Some industrial-vendor tenants use numeric codes
        // ("12345"). They're valid slugs.
        validate_slug("12345").unwrap();
    }

    #[test]
    fn input_round_trips_through_serde() {
        // The validated input is what the tenant-creation RPC
        // serializes upstream; pin the wire format stays
        // round-trippable for cross-stack consumption.
        let input = validate("acme", "Acme", "us-east-1", Industry::Automotive).unwrap();
        let json = serde_json::to_string(&input).unwrap();
        let back: OnboardingInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.slug, input.slug);
        assert_eq!(back.industry, input.industry);
    }
}
