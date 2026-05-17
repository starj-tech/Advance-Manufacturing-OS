//! Scan payload + classification — the data the controller path
//! sees after a scanner decodes whatever the operator presented.
//!
//! ## Why a wide [`ScanClass`] enum
//! Different downstream consumers care about different layers.
//! The compliance probe for FSMA Section 204 needs to distinguish
//! GS1-128 (mandatory for food traceability in the US since 2026)
//! from plain Code 128. The interlock layer doesn't care about
//! symbology — only "this is RFID UID, look it up against
//! `certifications`." Carrying both class AND bytes lets every
//! consumer filter at the level it cares about without re-parsing.
//!
//! Adding a class is intentional churn — the kebab-case slug is
//! persisted into the audit row, so a typo is a schema migration
//! and a rename is a wire-format break. The taxonomy here picks
//! the symbologies that show up in food / automotive / pharma in
//! the next 12 months and defers the long tail.

use aether_core::ScannerId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Auto-id symbology / RF protocol classification. Every variant
/// maps to one physical reading technique; the variant set is
/// closed and additions are deliberate (kebab-case slug is
/// persisted into the audit ledger).
///
/// Reading conventions for the data layout:
///   * `Code128`, `Ean13`, `Gs1_128`, `Itf14` — 1D linear bar
///     codes; the decoded payload is the printed text.
///   * `QrCode`, `DataMatrix`, `Aztec`, `Pdf417` — 2D codes;
///     payload is bytes in the symbology's character set, usually
///     UTF-8 in practice.
///   * `RfidIso14443a`, `RfidIso14443b` — HF (13.56 MHz) cards
///     (e.g. MIFARE). Payload is the UID + optional sector data.
///   * `RfidIso15693` — vicinity-range HF tags, payload is UID +
///     optional user memory.
///   * `Uhf` — passive UHF 860-960 MHz EPC tags (industrial pallet
///     tracking). Payload is the EPC bytes.
///   * `NfcTypeA` — generic NFC Type 1-5 reads. Payload is NDEF
///     records when present, raw UID otherwise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanClass {
    // 1D barcodes
    Code128,
    Ean13,
    Gs1_128,
    Itf14,
    // 2D codes
    QrCode,
    DataMatrix,
    Aztec,
    Pdf417,
    // RFID
    RfidIso14443a,
    RfidIso14443b,
    RfidIso15693,
    Uhf,
    // NFC
    NfcTypeA,
}

impl ScanClass {
    /// Kebab-case slug for audit-row persistence and log lines.
    /// Renaming a variant is a schema migration — the wire format
    /// is part of the contract with the compliance probes that
    /// filter by class.
    pub fn slug(&self) -> &'static str {
        match self {
            ScanClass::Code128 => "code-128",
            ScanClass::Ean13 => "ean-13",
            ScanClass::Gs1_128 => "gs1-128",
            ScanClass::Itf14 => "itf-14",
            ScanClass::QrCode => "qr-code",
            ScanClass::DataMatrix => "data-matrix",
            ScanClass::Aztec => "aztec",
            ScanClass::Pdf417 => "pdf-417",
            ScanClass::RfidIso14443a => "rfid-iso-14443-a",
            ScanClass::RfidIso14443b => "rfid-iso-14443-b",
            ScanClass::RfidIso15693 => "rfid-iso-15693",
            ScanClass::Uhf => "uhf",
            ScanClass::NfcTypeA => "nfc-type-a",
        }
    }

    /// Whether this classification's payload is structured text
    /// (UTF-8 in practice). Helps downstream code decide whether
    /// to `String::from_utf8` the payload bytes or keep them
    /// opaque. RFID UIDs are bytes; barcode contents are text.
    pub fn payload_is_text(&self) -> bool {
        matches!(
            self,
            ScanClass::Code128
                | ScanClass::Ean13
                | ScanClass::Gs1_128
                | ScanClass::Itf14
                | ScanClass::QrCode
                | ScanClass::DataMatrix
                | ScanClass::Aztec
                | ScanClass::Pdf417
        )
    }
}

/// Decoded scan payload with provenance. The `bytes` payload is
/// the raw decoded data — UTF-8 text for symbology codes, binary
/// UID and optional memory for RFID/NFC. Provenance fields
/// (`scanner_id`, `read_at`) let the audit ledger answer "which
/// scanner read this lot number and when," which the FSMA 204 and
/// ISO 22000 compliance probes both rely on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScannedPayload {
    pub scanner_id: ScannerId,
    pub read_at: DateTime<Utc>,
    pub class: ScanClass,
    pub bytes: Vec<u8>,
    /// Vendor confidence 0..=1 where the reader exposes one
    /// (Cognex Dataman, Honeywell N6603, Zebra DS series). `None`
    /// for readers without a confidence model — most RFID/NFC
    /// tags either read or don't.
    pub confidence: Option<f32>,
}

impl ScannedPayload {
    /// Convenience: text content if `class.payload_is_text()` and
    /// the bytes parse as UTF-8. Returns `None` for binary classes
    /// or invalid UTF-8 (which is a scanner bug or a tampered
    /// symbology — surface it explicitly rather than lossy-
    /// decoding into a partial string).
    pub fn as_text(&self) -> Option<&str> {
        if !self.class.payload_is_text() {
            return None;
        }
        std::str::from_utf8(&self.bytes).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_distinct_and_kebab_cased() {
        // The slug is persisted; collisions or non-kebab spelling
        // would corrupt the audit schema.
        let classes = [
            ScanClass::Code128,
            ScanClass::Ean13,
            ScanClass::Gs1_128,
            ScanClass::Itf14,
            ScanClass::QrCode,
            ScanClass::DataMatrix,
            ScanClass::Aztec,
            ScanClass::Pdf417,
            ScanClass::RfidIso14443a,
            ScanClass::RfidIso14443b,
            ScanClass::RfidIso15693,
            ScanClass::Uhf,
            ScanClass::NfcTypeA,
        ];
        let mut slugs: Vec<&'static str> = classes.iter().map(|k| k.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before, "all slugs distinct");
        for s in &slugs {
            assert!(!s.contains('_'), "kebab-case: {s}");
            assert_eq!(s.to_lowercase(), **s, "lowercase: {s}");
        }
    }

    #[test]
    fn text_payload_classification_separates_symbology_from_rfid() {
        // The 1D / 2D classes carry printed text; RFID / NFC
        // carry binary UIDs. Pin the boundary so the
        // `as_text()` helper stays sane.
        assert!(ScanClass::Code128.payload_is_text());
        assert!(ScanClass::QrCode.payload_is_text());
        assert!(!ScanClass::RfidIso14443a.payload_is_text());
        assert!(!ScanClass::NfcTypeA.payload_is_text());
    }

    #[test]
    fn as_text_returns_utf8_for_text_class() {
        let payload = ScannedPayload {
            scanner_id: ScannerId::new(),
            read_at: Utc::now(),
            class: ScanClass::QrCode,
            bytes: b"LOT-2026-05-17-007".to_vec(),
            confidence: Some(0.98),
        };
        assert_eq!(payload.as_text(), Some("LOT-2026-05-17-007"));
    }

    #[test]
    fn as_text_is_none_for_binary_class_even_if_bytes_are_valid_utf8() {
        // An RFID UID that happens to be valid UTF-8 should NOT
        // be surfaced as text — the class is the source of truth.
        let payload = ScannedPayload {
            scanner_id: ScannerId::new(),
            read_at: Utc::now(),
            class: ScanClass::RfidIso15693,
            bytes: b"ABCD1234".to_vec(),
            confidence: None,
        };
        assert!(payload.as_text().is_none());
    }

    #[test]
    fn as_text_is_none_for_invalid_utf8_on_text_class() {
        // A barcode scanner that returns bytes that aren't UTF-8
        // is a scanner bug — surface it as None rather than
        // lossy-decoding into a partial string.
        let payload = ScannedPayload {
            scanner_id: ScannerId::new(),
            read_at: Utc::now(),
            class: ScanClass::Code128,
            bytes: vec![0xFF, 0xFE, 0xFD], // invalid UTF-8
            confidence: None,
        };
        assert!(payload.as_text().is_none());
    }

    #[test]
    fn gs1_128_slug_uses_dash_not_underscore() {
        // The variant name uses an underscore (Gs1_128) because
        // Rust doesn't allow leading digits in identifiers and
        // `Gs1128` would collide with the convention. The serde
        // slug MUST still be kebab-case.
        let s = serde_json::to_string(&ScanClass::Gs1_128).unwrap();
        assert_eq!(s, "\"gs1-128\"");
        let back: ScanClass = serde_json::from_str("\"gs1-128\"").unwrap();
        assert_eq!(back, ScanClass::Gs1_128);
    }

    #[test]
    fn payload_round_trips_through_serde() {
        // The `bytes` field is Vec<u8>; default serde encoding
        // emits it as a JSON array of integers (not base64).
        // That's fine for ledger payloads — the table column is
        // BYTEA on the server side, and the JSON-array form
        // travels cleanly through the outbox.
        let payload = ScannedPayload {
            scanner_id: ScannerId::new(),
            read_at: Utc::now(),
            class: ScanClass::Uhf,
            bytes: vec![0x30, 0x14, 0x91, 0x1A],
            confidence: Some(0.87),
        };
        let s = serde_json::to_string(&payload).unwrap();
        let back: ScannedPayload = serde_json::from_str(&s).unwrap();
        assert_eq!(back, payload);
    }
}
