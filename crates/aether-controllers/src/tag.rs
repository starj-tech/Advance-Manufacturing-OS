//! `TagAddress` — typed wrapper around the vendor-specific tag
//! symbol string.
//!
//! ## Why a wrapper and not just `String`
//! Two reasons:
//!   1. Type safety. A function that takes `TagAddress` can't be
//!      accidentally called with a value string. Same pattern as
//!      the `*Id` newtypes in `aether-core`.
//!   2. A single validation point. Vendor syntaxes vary (OPC-UA
//!      `ns=2;s=...`, IEC 61131 `%MX0.0`, S7 `DB1.DBX0.0`,
//!      EtherNet/IP `Tag_Name.Field`), but ALL of them are
//!      non-empty and have a reasonable max length. Catching
//!      empty / oversized addresses at construction means the
//!      controller impl never has to second-guess the input.
//!
//! ## Why no parsing
//! Address parsing is vendor-specific. Each `Controller` impl
//! parses the string in the form it understands; the trait
//! surface only sees the wrapper. Forcing a parse here would
//! either lock us into one vendor's syntax or balloon into a
//! union-of-everyone's-grammar (which guarantees drift).

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Soft upper bound on address length. OPC-UA NodeIds can in
/// theory be quite long, but anything beyond 1 KiB is almost
/// certainly a bug or attempted misuse (untrusted input piped
/// into a tag write). Caps the per-call audit-row size as a side
/// benefit.
pub const MAX_TAG_ADDRESS_LEN: usize = 1024;

#[derive(Debug, Error)]
pub enum TagAddressError {
    #[error("tag address must not be empty")]
    Empty,
    #[error("tag address exceeds max length ({0} > {MAX_TAG_ADDRESS_LEN})")]
    TooLong(usize),
    /// Control characters in tag addresses are almost always a
    /// bug (copy-paste with embedded newline). Reject loudly
    /// rather than letting them corrupt log lines downstream.
    #[error("tag address contains control character at byte {0}")]
    ContainsControl(usize),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TagAddress(String);

impl TagAddress {
    /// Construct, validating non-empty + length-bounded +
    /// no-control-chars.
    pub fn new(s: impl Into<String>) -> Result<Self, TagAddressError> {
        let s = s.into();
        if s.is_empty() {
            return Err(TagAddressError::Empty);
        }
        if s.len() > MAX_TAG_ADDRESS_LEN {
            return Err(TagAddressError::TooLong(s.len()));
        }
        if let Some(idx) = s.bytes().position(|b| b < 0x20 || b == 0x7F) {
            return Err(TagAddressError::ContainsControl(idx));
        }
        Ok(Self(s))
    }

    /// Borrow the underlying string. Vendor-specific parsing
    /// happens here (inside the controller impl).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TagAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcua_nodeid_round_trips() {
        let a = TagAddress::new("ns=2;s=Channel1.Device1.Coil0").unwrap();
        assert_eq!(a.as_str(), "ns=2;s=Channel1.Device1.Coil0");
        assert_eq!(format!("{a}"), "ns=2;s=Channel1.Device1.Coil0");
    }

    #[test]
    fn iec_61131_address_round_trips() {
        let a = TagAddress::new("%MX0.0").unwrap();
        assert_eq!(a.as_str(), "%MX0.0");
    }

    #[test]
    fn s7_db_address_round_trips() {
        let a = TagAddress::new("DB1.DBX0.0").unwrap();
        assert_eq!(a.as_str(), "DB1.DBX0.0");
    }

    #[test]
    fn empty_address_is_rejected_with_typed_error() {
        let err = TagAddress::new("").unwrap_err();
        assert!(matches!(err, TagAddressError::Empty));
    }

    #[test]
    fn oversized_address_is_rejected_with_typed_error() {
        let huge = "A".repeat(MAX_TAG_ADDRESS_LEN + 1);
        let err = TagAddress::new(huge).unwrap_err();
        assert!(matches!(err, TagAddressError::TooLong(_)));
    }

    #[test]
    fn address_at_max_len_is_accepted_inclusive() {
        // Boundary: exactly MAX is fine; one more byte is not.
        let max = "A".repeat(MAX_TAG_ADDRESS_LEN);
        TagAddress::new(max).expect("max-length address should be accepted");
    }

    #[test]
    fn newline_in_address_is_rejected() {
        let err = TagAddress::new("DB1.DBX0.0\n").unwrap_err();
        assert!(matches!(err, TagAddressError::ContainsControl(_)));
    }

    #[test]
    fn null_byte_in_address_is_rejected() {
        let err = TagAddress::new("a\0b").unwrap_err();
        assert!(matches!(err, TagAddressError::ContainsControl(1)));
    }

    #[test]
    fn serde_round_trip_uses_transparent_string() {
        let a = TagAddress::new("ns=2;s=Plant.Reactor1.Temp").unwrap();
        let s = serde_json::to_string(&a).unwrap();
        // Transparent: serializes as the inner string, not a
        // wrapper object.
        assert_eq!(s, "\"ns=2;s=Plant.Reactor1.Temp\"");
        let back: TagAddress = serde_json::from_str(&s).unwrap();
        assert_eq!(back, a);
    }
}
