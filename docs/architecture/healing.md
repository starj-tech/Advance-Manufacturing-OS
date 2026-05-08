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
