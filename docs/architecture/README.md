# AETHER-OS — Architecture

This directory holds the deep-dive on how AETHER-OS is built. The seven
pillars introduced in the top-level README are each documented here.

## Pillars

| Pillar | Document | Implementation entry point |
|---|---|---|
| Hybrid-Native Monolith | _(see top-level README)_ | `apps/desktop/src-tauri/src/lib.rs` |
| Local-First Sync | [sync.md](./sync.md) | `crates/aether-sync/` |
| Zero-Knowledge Encryption | [crypto.md](./crypto.md) | `crates/aether-crypto/` |
| Dynamic Shell | [dynamic-shell.md](./dynamic-shell.md) | `apps/desktop/src/core/shell-switcher.tsx` |
| Module Injection | [modules.md](./modules.md) | `crates/aether-modules/`, `packages/module-sdk/` |
| Industrial Protocols | [protocols.md](./protocols.md) | `crates/aether-protocols/`, `crates/aether-opcua/`, `crates/aether-mqtt/` |
| Safety (Employee shell) | [safety.md](./safety.md) | `crates/aether-safety/`, `packages/glove-kit/` |

## Advanced Moats

| Moat | Document | Implementation entry point |
|---|---|---|
| Zero-Config IoT Discovery | [discovery.md](./discovery.md) | `crates/aether-discovery/` |
| Neural Auto-Healing | [healing.md](./healing.md) | `crates/aether-healing/`, `supabase/functions/healing-suggest/` |
| Smart Interlock Safety | [interlock.md](./interlock.md) | `crates/aether-safety/src/interlock.rs`, `supabase/migrations/0004_interlock.sql` |
| Global Supply Chain Hedging | [hedging.md](./hedging.md) | `crates/aether-hedging/`, `supabase/functions/commodity-feed/` |
| Multi-Lingual & Multi-Currency | [i18n.md](./i18n.md) | `crates/aether-money/`, `packages/i18n/` |
| Universal Module (industry logic injection) | [industry.md](./industry.md) | `crates/aether-industry/`, `supabase/migrations/0007_industry.sql` |
| Instant Compliance Attestation | [compliance.md](./compliance.md) | `crates/aether-compliance/`, `supabase/functions/compliance-attest/`, `supabase/migrations/0008_compliance.sql` |
| Self-Healing for Managers | [healing.md](./healing.md#manager-facing-surface) | `apps/desktop/src/shells/manager/SupportPage.tsx`, `commands::healing::healing_manager_summary` |

## Reading order

If you are new to the codebase:

1. Top-level `README.md` — what this is
2. `dynamic-shell.md` — how the UI is structured
3. `sync.md` — how data flows between SQLite and Supabase
4. `crypto.md` — what the server can and cannot see
5. `protocols.md` — how factory hardware enters the system
6. `modules.md` — how features are injected without redeploys
7. `safety.md` — how Employee Shell handles emergencies offline

## Skeleton vs. real

This PR ships skeletons (interfaces + tests + scaffolding). Each
follow-up PR fills in one pillar deeply:

- **PR #2** — sync engine + zero-knowledge auth
- **PR #3** — OPC-UA / MQTT bridges + telemetry pipeline
- **PR #4** — module loader + sample module
- **PR #5** — safety subsystem (SOS broadcast, geofence)
- **PR #6** — Digital Twin viewer (executive shell)
- **PR #7** — analytics module (predictive maintenance, profitability projections)
