# AETHER-OS — Architecture

This directory holds the deep-dive on how AETHER-OS is built. The seven
pillars introduced in the top-level README are each documented here.

> **Note on these deep-dives.** AETHER-OS is now a fully web-based product
> (browser SPA + Supabase Postgres + Deno Edge Functions). Several of the
> documents below were originally written against a native Rust
> implementation that has since been retired. Their _design intent_ —
> data flows, algorithms, verdict semantics, hash-chain layouts — remains
> the specification we implement against; treat any `crates/aether-*`
> reference as the design source for the equivalent TypeScript module or
> Edge Function. The established server-side template is
> `supabase/functions/compliance-attest`.

## Pillars

| Pillar                    | Document                               | Implementation entry point                               |
| ------------------------- | -------------------------------------- | -------------------------------------------------------- |
| Web-First SPA             | _(see top-level README)_               | `apps/demo/`                                             |
| Cloud-Backed Sync         | [sync.md](./sync.md)                   | `supabase/` (design: sync.md)                            |
| Zero-Knowledge Encryption | [crypto.md](./crypto.md)               | Web Crypto in SPA (design: crypto.md)                    |
| Dynamic Shell             | [dynamic-shell.md](./dynamic-shell.md) | `apps/demo/src/core/shell-switcher.tsx`                  |
| Module Injection          | [modules.md](./modules.md)             | `packages/module-sdk/`, Edge Function verify             |
| Industrial Protocols      | [protocols.md](./protocols.md)         | edge/gateway → Supabase (design: protocols.md)           |
| Safety (Employee shell)   | [safety.md](./safety.md)               | `packages/glove-kit/`, Supabase Realtime + Edge Function |

## Advanced Moats

| Moat                                        | Document                                          | Implementation entry point                                                      |
| ------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------- |
| Zero-Config IoT Discovery                   | [discovery.md](./discovery.md)                    | gateway service (design: discovery.md)                                          |
| Neural Auto-Healing                         | [healing.md](./healing.md)                        | `supabase/functions/healing-suggest/`                                           |
| Smart Interlock Safety                      | [interlock.md](./interlock.md)                    | Edge Function + `supabase/migrations/` (design: interlock.md)                   |
| Global Supply Chain Hedging                 | [hedging.md](./hedging.md)                        | `supabase/functions/commodity-feed/`                                            |
| Multi-Lingual & Multi-Currency              | [i18n.md](./i18n.md)                              | `packages/i18n/`                                                                |
| Universal Module (industry logic injection) | [industry.md](./industry.md)                      | `supabase/migrations/` + SPA capability provider                                |
| Instant Compliance Attestation              | [compliance.md](./compliance.md)                  | `supabase/functions/compliance-attest/`, `supabase/migrations/`                 |
| Self-Healing for Managers                   | [healing.md](./healing.md#manager-facing-surface) | `apps/demo/src/shells/manager/SupportPage.tsx`, `healing-summary` Edge Function |

## Reading order

If you are new to the codebase:

1. Top-level `README.md` — what this is
2. `dynamic-shell.md` — how the UI is structured
3. `sync.md` — how data flows between the client and Supabase
4. `crypto.md` — what the server can and cannot see
5. `protocols.md` — how factory hardware enters the system
6. `modules.md` — how features are injected without redeploys
7. `safety.md` — how Employee Shell handles emergencies
