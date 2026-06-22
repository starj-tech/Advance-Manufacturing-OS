-- Actuator commands — audit row per bidirectional dispatch.
--
-- Pairs with `crates/aether-actuators` (V1) + `aether-vision` (V2-V4) +
-- `aether-robotics` (V5-V8). Every command that flows through the
-- permit-gated `Actuator::dispatch` path writes a row here so the
-- audit story is "for every robot motion / camera capture / AGV
-- dispatch, the operator can prove who issued it, what the interlock
-- verdict was, and what the actuator reported back."
--
-- Distinct from `audit_log` (broad, free-form): this table is
-- typed, ordered, and indexed for the operational query
-- "what's stuck in_flight on machine X" — which the existing
-- `SyncStuckHealer` already polls for stuck outbox entries and
-- the Manager UI surfaces in the support page.
--
-- Status state machine (transitions enforced in app code, not via
-- a Postgres trigger so offline-first clients can transition
-- locally before syncing):
--     pending → in_flight → completed
--                       └ → failed
--                       └ → preempted
--   Terminal states (completed / failed / preempted) never transition
--   further. `REVOKE UPDATE` on terminal rows is not enforced here
--   because the audit grade comes from the HLC-ordered insert ledger
--   in `audit_log`, not from per-row mutability constraints — but the
--   RLS `scoped_update_actuator_commands` policy below only allows
--   transitions to non-terminal predecessors, which catches the
--   common bug.

CREATE TABLE actuator_commands (
    -- Stable command id. Same value flows through the outbox as
    -- `op_id` so a stuck command can be cross-referenced by id from
    -- either side without joining tables.
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- The actuator that was commanded. Not a FK because actuator
    -- identity is owned by the runtime (it's `ActuatorId(Uuid)` from
    -- aether-core, materialized when the device is bound) and we
    -- don't always have a Postgres row for every actuator (e.g.
    -- ephemeral mock actuators in test harnesses).
    actuator_id     UUID NOT NULL,
    -- Machine row the actuator belongs to. Optional — a discovered-
    -- but-unbound camera might fire test captures before it's mapped
    -- to a machine row.
    machine_id      UUID REFERENCES machines(id) ON DELETE SET NULL,
    -- Operator who issued the command. Optional only for system-
    -- initiated commands (e.g. auto-healing's "halt then restart").
    issued_by       UUID,

    -- Kebab-case CommandKind slug. Pinned in
    -- `aether_actuators::command::CommandKind::slug()` — capture,
    -- move-joint, move-linear, dispatch-job, halt. Renaming a
    -- variant in code is a schema migration.
    command_kind    TEXT NOT NULL
                    CHECK (command_kind IN (
                        'capture', 'move-joint', 'move-linear',
                        'dispatch-job', 'halt'
                    )),

    -- Command-specific arguments. JSONB in development; on the wire
    -- this is the same shape as `ActuatorCommand` serde JSON. When
    -- the row arrives via the outbox with `encrypted=true`, this
    -- column holds the *plaintext* after the sync engine has
    -- unwrapped the envelope — the encrypted form only exists in
    -- the outbox.payload BYTEA. Sensitive frames (vision Sensitive
    -- privacy class) wrap the payload before the outbox, so the
    -- server never sees the bytes.
    payload         JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Lifecycle.
    status          TEXT NOT NULL
                    CHECK (status IN (
                        'pending', 'in-flight', 'completed',
                        'failed', 'preempted'
                    )),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,

    -- For forensic linkage back to the interlock verdict that
    -- minted this command's permit. The permit itself is not
    -- persisted (single-use, TTL 5s, no value once consumed); this
    -- captures only its UUID so an operator can join with
    -- `interlock_events` by ad-hoc query.
    permit_id       UUID,

    -- Dispatch outcome. Captures the V1 `ActuatorResult.data` JSON
    -- for completed commands — frame UUID for captures, end pose
    -- for moves, route status for AGV dispatches. NULL for non-
    -- terminal states.
    result          JSONB,

    -- Typed error variant when status = 'failed'. Mirrors the
    -- ActuatorError variants but as TEXT for cross-language
    -- consumption. NULL otherwise.
    error_class     TEXT,
    error_detail    TEXT,

    -- E-stop linkage when status = 'preempted'. The dispatch was
    -- cancelled by an EstopSignal trip (V8); this captures *when*
    -- so the operator can correlate with the relay-side fault log.
    estop_tripped_at TIMESTAMPTZ,

    -- HLC for sync ordering. Same wire format as other replicated
    -- entities — `<wall_ms>.<logical>.<node>`.
    hlc             TEXT NOT NULL
);

CREATE INDEX idx_actuator_commands_tenant_machine_created
    ON actuator_commands(tenant_id, machine_id, created_at DESC);

-- The "what's stuck" operational query — surfaces in-flight rows
-- older than the SyncStuckHealer's threshold. The partial index
-- keeps it tiny since terminal rows dominate the table after a
-- short while.
CREATE INDEX idx_actuator_commands_in_flight
    ON actuator_commands(tenant_id, started_at)
    WHERE status = 'in-flight';

-- Per-actuator "what was the most recent thing it did" — supports
-- the Manager Machines page showing latest command status.
CREATE INDEX idx_actuator_commands_actuator_recent
    ON actuator_commands(actuator_id, created_at DESC);

ALTER TABLE actuator_commands ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_select_actuator_commands
    ON actuator_commands
    FOR SELECT USING (tenant_id = current_tenant());

CREATE POLICY scoped_insert_actuator_commands
    ON actuator_commands
    FOR INSERT WITH CHECK (tenant_id = current_tenant());

-- Allow only non-terminal → next transitions. Terminal rows
-- (completed / failed / preempted) cannot be mutated further;
-- attempting to re-open one is a programming bug surfaced as a
-- RLS violation rather than silent overwrite.
CREATE POLICY scoped_update_actuator_commands
    ON actuator_commands
    FOR UPDATE
    USING (
        tenant_id = current_tenant()
        AND status IN ('pending', 'in-flight')
    )
    WITH CHECK (
        tenant_id = current_tenant()
        AND status IN ('in-flight', 'completed', 'failed', 'preempted')
    );

-- Audit-grade: no delete after insert.
REVOKE DELETE ON actuator_commands FROM authenticated;
