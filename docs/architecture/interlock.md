# Smart Interlock Safety

> Status: skeleton with full evaluation logic + Supabase schema.
> Real protocol writes (Modbus coil / OPC-UA write) land with PR #5.

## Why this is different from a software permission

Most "permission" systems gate the **UI**. Smart Interlock gates the
**machine**. The motor's safety relay only energizes when AETHER-OS
writes `permit = true` to the relay's enable input over the protocol
bridge. Bypass the UI? The motor still won't run.

## Logic

`crates/aether-safety/src/interlock.rs::evaluate`:

```
if user is in lockout       → DenyLockout
elif machine has fault      → DenyMachineFault
elif user missing any cert  → DenyMissingCert(codes)
elif any required cert exp. → DenyExpiredCert(codes)
else                        → Allow
```

`InterlockController::request_unlock` runs the evaluator and then
writes the verdict to the safety-relay coil. Every verdict — including
`Allow` — is recorded in `interlock_events` for forensics and OSHA-style
audits.

## Schema

`supabase/migrations/0004_interlock.sql`:

- `certifications` — per-tenant (user_id, cert_code, issued/expires/revoked)
- `machine_required_certs` — required cert codes per machine
- `machine_permits` — protocol target for the relay coil + last-set state
- `interlock_events` — append-only audit, no UPDATE/DELETE grants

## Data flow

```
Operator badge tap on shop-floor terminal
                │
                ▼
      AETHER-OS Employee shell
                │ invoke('interlock_request_unlock', ...)
                ▼
   crates/aether-safety::evaluate
                │ verdict
                ▼
   protocol bridge writes coil
                │
                ▼
   record into interlock_events
                │
                ▼
   safety relay closes / stays open
```

## Verification

- 6 unit tests in `interlock.rs` cover allow / missing / expired /
  revoked / lockout / machine-fault paths.
- E2E (PR #5): mock OPC-UA server with permit coil + simulated badge
  tap → verify coil state matches verdict.
