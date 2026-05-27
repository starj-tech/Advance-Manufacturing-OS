# Local-First Sync

> Status: skeleton. Reconciler + property-based tests land in PR #2.

SQLite (per-tenant file under `<app_data>/aether/<tenant_id>/main.db`) is
the runtime source of truth. Supabase Postgres is the eventually
consistent record of authority. The factory keeps operating when WAN is
down; once the cloud is reachable, changes converge.

## Pattern

Hybrid:

- **Outbox + Hybrid Logical Clock (HLC)** for transactional rows.
- **Automerge CRDT** (encrypted blob) for collaborative documents (BOM, SOP).

CRDT for _everything_ is rejected because counter constraints
(`qty_on_hand >= 0`) and atomic state transitions
(`work_orders.status`) are not expressible in commutative merges
without sacrificing auditability.

## Conflict policy by domain

| Domain                  | Policy                  | Why                                                                                                                                                                               |
| ----------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `materials.qty_on_hand` | Reject + surface        | Inventory integrity beats availability. Writes go via server RPC `inventory_adjust(delta)` which enforces `qty + delta >= 0` atomically. Outbox stores deltas, not target values. |
| `work_orders.status`    | Server transition       | RPC `wo_transition(id, from, to)` checks `current_state = from`.                                                                                                                  |
| `boms`, `sop_drafts`    | Automerge merge         | Tree-shaped, frequently co-edited, fits CRDT well.                                                                                                                                |
| Default                 | Last-writer-wins by HLC | Sufficient for non-critical fields.                                                                                                                                               |

Codified in `crates/aether-sync/src/reconcile.rs::policy_for`.

## HLC wire format

`<wall_ms>.<logical>.<node>` — sortable as a string, parseable in both
Rust and TypeScript. Node id is the device's per-install UUID.

## Outbox queue (SQLite)

```
sync_outbox
├── op_id       UUIDv7 (sortable)
├── entity      'work_orders' | 'materials' | ...
├── entity_id   ULID/UUIDv7 of the row
├── op          'insert' | 'update' | 'delete' | 'crdt_patch'
├── payload     ciphertext if encrypted; JSON otherwise
├── hlc_ts      HLC at write time
├── parent_hlc  HLC of the version edited (OCC)
├── encrypted   0|1
├── attempts    backoff counter
├── last_error
└── created_at
```

## Push / pull loop (PR #2)

```
push_loop():
  for batch in outbox.poll(limit=100):
    resp = supabase.rpc('sync_apply_batch', batch)
    for c in resp.conflicts:
      handle by policy_for(c.entity)
    outbox.mark_done(batch.ids)

pull_loop():
  changes = supabase.rpc('sync_changes_since', last_pulled_hlc)
  for c in changes:
    apply_with_hlc(c)
```

## Verification

- Property-based test (`proptest`) in `aether-sync`: random op sequences
  on two clients converge to the same state on the server.
- Round-trip test SQLite ↔ Postgres for every entity in `rpc-contracts`
  to catch type-mapping drift.
