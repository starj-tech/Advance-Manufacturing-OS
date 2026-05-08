# Neural Auto-Healing

> Status: skeleton. Real healers + LLM-assisted suggestion service land in PR #5.

## Goals

- Detect runtime symptoms (panics, sync stalls, bridge disconnect loops,
  SQLite corruption signals, telemetry overflow).
- Apply a remediation policy automatically when confidence is high.
- Audit every action append-only so operators can reconstruct what
  happened on their behalf.
- Escalate to humans cleanly when symptoms are non-mechanical or when a
  destructive action is required.

## Components

`crates/aether-healing`:

- `Symptom` — `{ source, kind, detail, severity }`. Everything subject
  to healing is described in this shape.
- `Healer` — trait. `diagnose(symptom)` → `Option<Diagnosis>`,
  `apply(policy)` → `String`.
- `HealingPolicy` — restart service, rollback module, recover SQLite,
  trim telemetry, escalate. `is_destructive()` flags actions that
  require an extra confirmation tier.
- `HealingLedger` — append-only event log mirrored to `audit_log` so the
  cloud audit trail is unbroken.

## Dispatch

```
                    Symptom
                       │
              ┌────────▼─────────┐
              │ Healer chain     │  (trait objects, ordered)
              └────────┬─────────┘
                       │ best Diagnosis (highest confidence)
                       ▼
        ┌─────────────────────────────────┐
        │ destructive?  yes  → require    │
        │                       cloud OK   │
        │                no   → apply now │
        └─────────────────────────────────┘
                       │
                       ▼
                 HealingLedger
```

## LLM tier (PR #5)

When local rule-based healers can't reach a confident diagnosis, the
symptom is forwarded to `supabase/functions/healing-suggest`. That
function calls Claude with prompt caching keyed on (`source`, `kind`)
so repeat symptoms are essentially free. The response is shaped as a
`Diagnosis`; the local engine still chooses whether to apply.

## Safety boundaries

- `HealingPolicy::TrimTelemetry` and `RecoverSqlite` are flagged
  destructive — they require a second-factor confirmation from the cloud
  function before being applied.
- `EscalateToHuman` is always preferred over guessing when severity is
  critical (severity ≥ 3) and confidence < 0.7.
- Every action is reversible-where-possible: rollbacks restore module
  state via `tenant_modules.pinned_version` so we never delete data.

## Manager-facing surface

The Manager Shell exposes a curated view at `/manager/support`. Raw
symptoms + policies are translated into plain language so a plant
manager doesn't need to call IT to understand what just happened:

- **auto-fixed** cards summarize a self-healing event in one paragraph
  (e.g. "Sync queue caught up by itself" with a description of how
  long the network was offline and that no data was lost).
- **watching** cards surface intermittent issues that don't yet need
  intervention but might if they recur ("PRESS-01 OPC-UA reconnect
  storm — paused for 2 minutes; check the cabinet cable if it
  repeats").
- **needs-you** cards request a one-click confirmation for destructive
  policies. The default-deny posture for `is_destructive()` actions
  ensures destructive remediation never happens without an explicit
  human "approve" — but each card is written in plain language so
  approving doesn't require a ticket to support.

The Tauri command `healing_manager_summary` returns these cards as
`Vec<ManagerHealingCard>`; the Manager Shell renders them in
`apps/desktop/src/shells/manager/SupportPage.tsx`.
