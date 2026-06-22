-- Operator events — append-only ledger for typed HMI interactions
-- drained from `aether_hmi::Hmi::pending_events()`.
--
-- Distinct from `actuator_commands` (V10): that table audits per-
-- command rows (operator/system ACTIONS). This one ledgers per-
-- gesture INTERACTIONS (operator inputs). A workflow analyst's
-- query "how many critical alarms got Acknowledged vs Cancelled
-- this shift" filters this table; "how many announcements did
-- the system push" filters actuator_commands. Splitting them
-- lets each access pattern stay cheap on its own index.
--
-- The flow: operator emits gesture/voice/tap → HMI surface
-- enqueues `OperatorEvent` → orchestrator calls `pending_events`
-- to drain → `operator_event::encode_event` produces an outbox
-- `Insert` → V10 outbox flushes upstream → server applies row.

CREATE TABLE operator_events (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- The HMI surface the event came from. Not a FK because
    -- HMI identity is owned by the runtime (`HmiId(Uuid)` from
    -- aether-core); not every surface has a Postgres row.
    hmi_id          UUID NOT NULL,
    -- Optional cross-reference to the operator's user_id where
    -- known (badge-bound sessions). NULL for un-authenticated
    -- pendant interactions on shared surfaces.
    user_id         UUID,
    -- Optional cross-reference to the machine context the
    -- interaction was associated with. NULL for system-wide
    -- gestures (alarm acknowledge from the home dashboard).
    machine_id      UUID REFERENCES machines(id) ON DELETE SET NULL,

    -- Kebab-case event kind slug. Pinned in
    -- `aether_hmi::OperatorEvent::slug()` — tap, swipe, voice,
    -- gaze-dwell, acknowledge, cancel. Renaming a variant in
    -- code is a schema migration.
    event_kind      TEXT NOT NULL
                    CHECK (event_kind IN (
                        'tap', 'swipe', 'voice',
                        'gaze-dwell', 'acknowledge', 'cancel'
                    )),

    -- Structured event payload as JSONB. Matches the
    -- `OperatorEvent` serde shape: `region` for Tap/Swipe/Gaze,
    -- `direction` for Swipe, `utterance` + `confidence` for
    -- Voice, `duration_ms` for GazeDwell. Acknowledge/Cancel
    -- have empty payloads (default `{}`).
    payload         JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Wall-time the surface stamped the event. Source of truth
    -- for ordering when the surface buffered the event before
    -- the orchestrator drained.
    occurred_at     TIMESTAMPTZ NOT NULL,
    -- Wall-time the server inserted the row. Server-side default
    -- `now()`; the gap measures end-to-end latency from gesture
    -- to ledger.
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- HLC for sync ordering — same wire format as other
    -- replicated entities.
    hlc             TEXT NOT NULL
);

-- "What did this HMI emit in the last shift" — workhorse query
-- for operator-interaction analytics. DESC matches the natural
-- "newest first" view in the UI.
CREATE INDEX idx_operator_events_hmi_occurred
    ON operator_events(tenant_id, hmi_id, occurred_at DESC);

-- "How many acknowledges this hour" — kind-filtered rollups
-- without scanning the whole table.
CREATE INDEX idx_operator_events_kind_occurred
    ON operator_events(tenant_id, event_kind, occurred_at DESC);

-- "Show me this operator's interactions" — the audit story for
-- a specific badge-bound session. Partial index keeps it tiny
-- when user_id is NULL (the common case).
CREATE INDEX idx_operator_events_user_occurred
    ON operator_events(tenant_id, user_id, occurred_at DESC)
    WHERE user_id IS NOT NULL;

ALTER TABLE operator_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_select_operator_events
    ON operator_events
    FOR SELECT USING (tenant_id = current_tenant());

CREATE POLICY scoped_insert_operator_events
    ON operator_events
    FOR INSERT WITH CHECK (tenant_id = current_tenant());

-- Audit-grade: no mutation after insert. "What did the operator
-- do during the incident" needs forever-valid answers.
REVOKE UPDATE, DELETE ON operator_events FROM authenticated;
