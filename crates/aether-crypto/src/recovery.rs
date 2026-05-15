//! BIP39 recovery phrases for tenant onboarding.
//!
//! When a tenant first onboards, AETHER-OS generates a 24-word BIP39
//! mnemonic that is displayed exactly once and which the owner is
//! required to back up. This phrase — combined with the tenant's UUID
//! as salt — derives the tenant's `MasterKey` via Argon2id (see
//! `kdf::derive_master_key`).
//!
//! ## Why BIP39
//!  * **Human-transcribable**: 24 English words from a fixed 2048-word
//!    list are dramatically easier to write down accurately than
//!    arbitrary hex; the checksum in the last word catches typos.
//!  * **Recoverable across devices**: anyone with the phrase + tenant
//!    UUID can re-derive the master key on a fresh install. The phrase
//!    is the disaster-recovery escape hatch when every enrolled device
//!    is lost.
//!  * **Industry standard**: every password manager, hardware wallet,
//!    and crypto product already supports it. Users transferring from
//!    other vaults can paste a phrase they already trust.
//!
//! ## Threat model
//! The phrase is the master secret. Anyone holding it can decrypt the
//! tenant's data even without access to an enrolled device. AETHER-OS
//! NEVER transmits or persists the phrase: it is shown once on screen,
//! the user is forced through a confirmation step (PR #2 onboarding
//! wizard), and the in-memory bytes are zeroized immediately.

use crate::kdf::{derive_master_key, MasterKey};
use crate::{CryptoError, CryptoResult};
use bip39::{Language, Mnemonic};
use rand_core::{OsRng, RngCore};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Number of entropy bits we generate. 256 bits → 24 BIP39 words.
/// Using the maximum length protects against future-quantum-leaning
/// adversaries; the user UX cost (24 vs 12 words) is acceptable given
/// the recovery flow is "once per tenant lifetime".
pub const PHRASE_ENTROPY_BITS: usize = 256;
const ENTROPY_BYTES: usize = PHRASE_ENTROPY_BITS / 8;

/// A validated BIP39 recovery phrase. The original mnemonic string is
/// held in [`Zeroizing`] so it is wiped from memory as soon as the
/// value is dropped — the onboarding flow MUST drop it the moment the
/// user has copied it down.
pub struct RecoveryPhrase {
    /// 24-word phrase, space-separated. Zeroized on drop.
    words: Zeroizing<String>,
    /// Underlying mnemonic (used only for `to_seed`). Always re-validated
    /// from `words` to avoid keeping derived material alive longer.
    mnemonic: Mnemonic,
}

impl RecoveryPhrase {
    /// Generate a fresh 24-word phrase using OS randomness.
    ///
    /// bip39 2.x removed `Mnemonic::generate(words)` in favor of
    /// `from_entropy`, which puts the burden of CSPRNG choice on the
    /// caller. We pull from the OS RNG via `rand_core::OsRng` so the
    /// entropy source matches the rest of the crypto layer (Ed25519
    /// signers, XChaCha20 nonces) — one chokepoint to vet for FIPS /
    /// audit.
    pub fn generate() -> CryptoResult<Self> {
        let mut entropy = Zeroizing::new([0u8; ENTROPY_BYTES]);
        OsRng.fill_bytes(entropy.as_mut_slice());
        let mnemonic = Mnemonic::from_entropy(entropy.as_slice())
            .map_err(|e| CryptoError::Kdf(format!("bip39 from_entropy: {e}")))?;
        let words = Zeroizing::new(mnemonic.to_string());
        Ok(Self { words, mnemonic })
    }

    /// Parse and validate a user-supplied phrase. Invalid checksum,
    /// unknown word, or wrong word count → `CryptoError::Kdf`. The
    /// validation catches the most common transcription mistakes
    /// before any expensive Argon2id work happens.
    pub fn parse(input: &str) -> CryptoResult<Self> {
        // bip39::parse_in_normalized handles internal whitespace
        // normalization but bails on leading / trailing whitespace, so
        // we strip those ourselves first.
        let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
            .map_err(|e| CryptoError::Kdf(format!("bip39 parse: {e}")))?;
        let words = Zeroizing::new(mnemonic.to_string());
        Ok(Self { words, mnemonic })
    }

    /// Number of words in the phrase (always 24 for `generate`, may be
    /// 12 / 15 / 18 / 21 / 24 when imported from another wallet).
    pub fn word_count(&self) -> usize {
        self.mnemonic.word_count()
    }

    /// Render the phrase for display. The caller MUST drop the returned
    /// `Zeroizing<String>` immediately after rendering so the
    /// transcript is wiped.
    pub fn display(&self) -> Zeroizing<String> {
        Zeroizing::new(self.words.as_str().to_string())
    }

    /// BIP39 seed bytes. The passphrase argument is the empty string —
    /// AETHER-OS doesn't add a secondary password on top of the phrase
    /// because the tenant UUID salt + Argon2id already provide the
    /// per-tenant uniqueness we need.
    fn to_seed(&self) -> Zeroizing<[u8; 64]> {
        Zeroizing::new(self.mnemonic.to_seed(""))
    }
}

impl std::fmt::Debug for RecoveryPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryPhrase")
            .field("word_count", &self.word_count())
            .field("words", &"***")
            .finish()
    }
}

/// Derive a tenant-bound `MasterKey` from a recovery phrase + tenant UUID.
///
/// The tenant UUID is the Argon2id salt, so the same recovery phrase
/// produces a *different* key per tenant. This is the property that
/// lets a service-provider hold many tenants' phrases (in an emergency
/// envelope, for example) without any tenant's key being derivable
/// from any other tenant's data.
///
/// The Argon2id parameters come from `kdf::derive_master_key`
/// (OWASP-recommended, ~200 ms on a modern laptop).
pub fn derive_master_key_for_tenant(
    phrase: &RecoveryPhrase,
    tenant_id: Uuid,
) -> CryptoResult<MasterKey> {
    let seed = phrase.to_seed();
    // tenant UUID is 16 bytes — exactly the Argon2id minimum salt our
    // `derive_master_key` enforces. No padding required.
    derive_master_key(seed.as_slice(), tenant_id.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_yields_24_words() {
        let p = RecoveryPhrase::generate().expect("generate");
        assert_eq!(p.word_count(), 24);
        let rendered = p.display();
        // 24 space-separated words.
        assert_eq!(rendered.split_whitespace().count(), 24);
    }

    #[test]
    fn round_trip_via_display_and_parse() {
        let original = RecoveryPhrase::generate().unwrap();
        let s = original.display();
        let parsed = RecoveryPhrase::parse(&s).expect("re-parse own output");
        assert_eq!(parsed.word_count(), original.word_count());
        assert_eq!(parsed.display().as_str(), original.display().as_str());
    }

    #[test]
    fn parse_normalizes_whitespace() {
        let p = RecoveryPhrase::generate().unwrap();
        let canonical = p.display().as_str().to_string();
        // Stretch the spacing to look like a clumsy paste from a notes app.
        let messy = format!("  {}  ", canonical.replace(' ', "   \t"));
        let parsed = RecoveryPhrase::parse(&messy).expect("whitespace tolerant");
        assert_eq!(parsed.display().as_str(), canonical);
    }

    #[test]
    fn parse_rejects_unknown_word() {
        let bad = "zzzzz ".repeat(24);
        let err = RecoveryPhrase::parse(bad.trim()).unwrap_err();
        assert!(matches!(err, CryptoError::Kdf(_)));
    }

    #[test]
    fn parse_rejects_wrong_word_count() {
        let only_12 = "abandon abandon abandon abandon abandon abandon \
                       abandon abandon abandon abandon abandon abandon";
        // 12-word phrases have a different checksum to be valid BIP39
        // mnemonics on their own — the all-`abandon` 12-word string is
        // a known-good 128-bit zero entropy, so it actually parses.
        // Hand the parser something that is *not* on a valid boundary:
        // 13 words is never a valid length.
        let invalid_len = format!("{only_12} abandon");
        let err = RecoveryPhrase::parse(&invalid_len).unwrap_err();
        assert!(matches!(err, CryptoError::Kdf(_)));
    }

    #[test]
    fn same_phrase_same_tenant_yields_same_key() {
        let phrase = RecoveryPhrase::generate().unwrap();
        let tenant = Uuid::new_v4();
        let a = derive_master_key_for_tenant(&phrase, tenant).unwrap();
        let b = derive_master_key_for_tenant(&phrase, tenant).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn same_phrase_different_tenant_yields_different_keys() {
        let phrase = RecoveryPhrase::generate().unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let key_a = derive_master_key_for_tenant(&phrase, tenant_a).unwrap();
        let key_b = derive_master_key_for_tenant(&phrase, tenant_b).unwrap();
        assert_ne!(
            key_a.as_bytes(),
            key_b.as_bytes(),
            "tenant salt isolation must hold"
        );
    }

    #[test]
    fn round_trip_via_parsed_phrase_preserves_master_key() {
        // The most important real-world property: an operator types a
        // phrase on a new device → the SAME master key is recovered.
        let original = RecoveryPhrase::generate().unwrap();
        let tenant = Uuid::new_v4();
        let key_original = derive_master_key_for_tenant(&original, tenant).unwrap();

        let typed_back = RecoveryPhrase::parse(original.display().as_str()).unwrap();
        let key_recovered = derive_master_key_for_tenant(&typed_back, tenant).unwrap();

        assert_eq!(
            key_original.as_bytes(),
            key_recovered.as_bytes(),
            "disaster recovery must round-trip the master key"
        );
    }

    #[test]
    fn debug_does_not_leak_phrase() {
        let p = RecoveryPhrase::generate().unwrap();
        let dbg = format!("{p:?}");
        assert!(dbg.contains("***"), "phrase data must be redacted");

        // The canonical 24-word string MUST NOT appear in the Debug
        // output. We can't check word-by-word because BIP39 words like
        // `very` and `word` legitimately occur as substrings of the
        // Debug struct name and field labels ("RecoveryPhrase",
        // "word_count"), producing a flaky false-positive ~1% of runs.
        // The full phrase string is what an attacker could meaningfully
        // exfiltrate from a Debug log.
        let phrase = p.display();
        assert!(
            !dbg.contains(phrase.as_str()),
            "Debug leaked the full phrase"
        );

        // Stronger: any two adjacent phrase words appearing together
        // would still be a partial leak. Redaction must collapse the
        // whole sequence, so no bigram survives.
        let words: Vec<&str> = phrase.split_whitespace().collect();
        for pair in words.windows(2) {
            let bigram = format!("{} {}", pair[0], pair[1]);
            assert!(!dbg.contains(&bigram), "Debug leaked bigram `{bigram}`");
        }
    }
}
