# AETHER-OS

> Industrial Operating System for advanced manufacturing — _Invisible yet Omnipresent_.

AETHER-OS adalah desktop-native Industrial OS yang dirancang menjadi sistem saraf pusat bagi pabrik manufaktur — dari workshop kecil hingga gigafactory. Satu binary, empat wajah (Developer, Executive, Manager, Employee), local-first dengan zero-knowledge encryption.

## Tujuh Pilar Arsitektur

1. **Hybrid-Native Monolith** — satu Tauri 2 binary; protokol industri (OPC-UA, MQTT, sensor passthrough) hidup di Rust, bukan di browser layer. Latency native, akses hardware tanpa kompromi.
2. **Local-First, Cloud-Later** — SQLite per-tenant adalah source of truth runtime. Pabrik tetap beroperasi penuh saat WAN down. Sync ke Supabase saat koneksi pulih, dengan reconciliation outbox + HLC + CRDT.
3. **Zero-Knowledge Encryption** — XChaCha20-Poly1305 + Argon2id. Master key tenant tidak pernah meninggalkan device. Server hanya melihat ciphertext untuk data sensitif (formula, finansial, IP).
4. **Dynamic Shell** — satu codebase yang bertransformasi berdasarkan role pengguna setelah otentikasi. Lazy chunks per shell, shared chrome konsisten, server-enforced permission via RLS.
5. **Module Injection** — fitur tambahan = data terdaftar + ditandatangani (ed25519). Hot-load di Web Worker dengan capability gating. Update tanpa stop produksi.
6. **Industrial Protocols Native** — OPC-UA via `opcua` crate, MQTT via `rumqttc`. Trait `Bridge` mengabstraksi sehingga domain layer tidak peduli apakah data datang dari Siemens S7 atau MQTT broker.
7. **Safety-First Employee Shell** — SOS multi-tier (mDNS broadcast → MQTT lokal → Supabase Realtime → SMS/Slack). Geofencing GPS+Wi-Fi+BLE. Glove-Friendly UI dengan 56dp touch targets dan AAA contrast.

## Quickstart

### Prasyarat

- Node.js ≥ 20 (lihat `.nvmrc`)
- pnpm ≥ 9
- Rust stable (lihat `rust-toolchain.toml`)
- Supabase CLI (untuk pengembangan lokal)
- Platform desktop dependencies untuk Tauri 2 — lihat https://v2.tauri.app/start/prerequisites/

### Setup

```bash
# Install dependencies
pnpm install
cargo build --workspace

# Bring up Supabase locally
supabase start
supabase db reset

# Run desktop app in dev mode
pnpm tauri:dev
```

### Mock OPC-UA Server (untuk dev/testing)

```bash
cargo run -p opcua-mock-server
# listens on opc.tcp://127.0.0.1:4840
```

### Lint & Test

```bash
pnpm lint
pnpm typecheck
pnpm test
cargo fmt --check && cargo clippy --workspace -- -D warnings
cargo test --workspace
```

## Layout Repositori

```
.
├── apps/desktop/             # Tauri 2 app utama
├── packages/                 # JS/TS shared packages
│   ├── ui-kit/               # Design system standar
│   ├── glove-kit/            # Touch-first variant utk Employee Shell
│   ├── shell-runtime/        # Dynamic Shell engine
│   ├── module-sdk/           # SDK utk vendor modul
│   ├── rpc-contracts/        # Shared Zod schemas + TS types
│   └── eslint-config/        # Shared lint config
├── crates/                   # Rust crates (Cargo workspace)
│   ├── aether-core/
│   ├── aether-db/
│   ├── aether-sync/
│   ├── aether-crypto/
│   ├── aether-opcua/
│   ├── aether-mqtt/
│   ├── aether-protocols/
│   ├── aether-modules/
│   ├── aether-safety/
│   └── aether-telemetry/
├── supabase/                 # Postgres migrations + Edge Functions
├── tools/                    # Developer tools (mock OPC-UA, module signer)
└── docs/architecture/        # Deep-dive arsitektur
```

## Status

PR ini mengirimkan **foundation skeleton**: seluruh plumbing arsitektur siap, implementasi mendalam tiap pilar (sync engine, crypto round-trip, OPC-UA real subscription, SOS broadcast, dll) datang di PR berikutnya.

Lihat `docs/architecture/README.md` untuk peta deep-dive.

## License

Proprietary © starj-tech
