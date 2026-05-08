# Zero-Knowledge Encryption

> Status: skeleton. Master key flow + per-OS keystore wiring land in PR #2.

## Threat model

- **Honest-but-curious server** for non-encrypted data (operational rows
  needed for RLS and realtime broadcast).
- **Untrusted server** for encrypted data: formulas, financial detail,
  PII, BOM/SOP documents.

Even with full server compromise, encrypted data must remain
unreadable to the attacker.

## Algorithms

| Function | Algorithm | Crate |
|---|---|---|
| AEAD | XChaCha20-Poly1305 (24-byte nonce) | `chacha20poly1305` 0.10 |
| KDF | Argon2id (m=64MB, t=3, p=1) | `argon2` 0.5 |
| Asymmetric (sealing) | X25519 + XSalsa20-Poly1305 | `crypto_box` 0.9 |
| Signing | Ed25519 | `ed25519-dalek` 2 |
| HKDF | HMAC-SHA256 | `hkdf` 0.12 |
| Blind index | HMAC-SHA256 → 16-byte tag | `hmac` 0.12 |

## Master key flow

1. **Onboarding** — tenant owner generates a 24-word recovery phrase
   (BIP39). `derive_master_key(phrase, tenant_id)` produces
   `TENANT_MASTER_KEY` via Argon2id. The phrase is shown once and the
   user is forced to back it up.
2. **Per-device enrollment** — the device registers a WebAuthn passkey
   with the server. The master key is wrapped with a device-bound key
   stored in the OS keychain (Stronghold fallback). Recovery phrase is
   zeroized from RAM after enrollment.
3. **Per-session unlock** — passkey assertion authorizes the keystore
   to unwrap the device blob. The master key lives in RAM (zeroized on
   lock or app exit).

Important: WebAuthn signatures are non-deterministic, so they cannot be
used directly as KDF input. Instead they unlock the OS keychain entry —
the same pattern used by 1Password / Bitwarden / Signal.

## Sub-key derivation

```
TENANT_MASTER_KEY ──HKDF(salt=tenant_id, info="data")  → DATA_DEK
                  ──HKDF(salt=tenant_id, info="search")→ BLIND_INDEX_KEY
                  ──HKDF(salt=tenant_id, info="wrap")  → KEY_WRAPPING_KEY
```

## Field-level vs row-level

| Data | Strategy |
|---|---|
| `formulas.recipe`, `financial_ledger.amount` | Field-level (ciphertext column + nonce) |
| `users.email`, PII | Field-level + blind index for login lookup |
| `work_orders` operational state | Plaintext (RLS-protected) |
| `telemetry` | Plaintext (server aggregation needed) |
| BOM document | Single ciphertext blob (Automerge bytes encrypted) |

Row-level encryption (whole row → blob) is rejected because it kills
RLS, indexing, realtime delta, and server-side aggregation.

## Search

- **Equality** — blind index (`HMAC_SHA256(BLIND_INDEX_KEY, normalize(value))[:16]`).
- **Range** — bucketization (per tier hash) when needed. OPE rejected
  due to documented statistical leakage.
- **Fuzzy** — client-side: pull subset, decrypt in Rust, FTS5 over
  decrypted cache.

## Key rotation

- DEK quarterly via `key_epoch` column + background re-encrypt batch.
- Master key only on suspected compromise: re-derive from a new recovery
  phrase, re-wrap all DEKs, dual-key decrypt window during migration.
