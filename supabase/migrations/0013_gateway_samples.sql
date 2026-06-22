-- Gateway samples — append-only telemetry ledger for samples
-- drained from `aether_gateway::Gateway::pending_samples()`.
--
-- Distinct from `actuator_commands` (V10): that table is the
-- per-command audit ledger; this one is the per-sample telemetry
-- ledger. Different cardinality (1-1000s/sec vs dozens/min),
-- different access pattern (gateway buffer history vs operator
-- action audit), different retention tier (hot-cold rollup for
-- telemetry vs audit-grade forever for commands).
--
-- The flow: gateway buffers samples while WAN is down → operator
-- (or scheduled task) calls `pending_samples` to drain →
-- `gateway_sample::encode_sample` produces an outbox `Insert` →
-- the V10 outbox flushes upstream → server applies row here.

CREATE TABLE gateway_samples (
    -- Stable sample id. Same value flows through the outbox as
    -- `op_id` so a stuck outbox entry can be cross-referenced
    -- by id with the row on the server without joining tables.
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- The gateway that buffered + forwarded the sample. Not a
    -- FK because gateway identity is owned by the runtime
    -- (`GatewayId(Uuid)` from aether-core); not every gateway
    -- has a Postgres row (e.g. ephemeral mock gateways in tests).
    gateway_id      UUID NOT NULL,
    -- Optional cross-reference to the machine the sample
    -- originated from, where the gateway knows. NULL for
    -- protocol-bridge gateways that don't bind upstream
    -- topics to machine rows.
    machine_id      UUID REFERENCES machines(id) ON DELETE SET NULL,

    -- Upstream topic / endpoint the gateway was forwarding to.
    -- Pinned at buffer-time on the client side; persisted here
    -- verbatim so a server-side reconfig doesn't retroactively
    -- rewrite history.
    topic           TEXT NOT NULL CHECK (length(topic) > 0),

    -- Raw payload bytes. Capped at 64 KiB on the client side
    -- (`aether_gateway::MAX_PAYLOAD_BYTES`) to match the buffer
    -- contract; the server also caps via the CHECK below in
    -- case a non-canonical client tries to push more.
    payload         BYTEA NOT NULL
                    CHECK (octet_length(payload) <= 65536),

    -- Wall-time at the gateway when it accepted the sample.
    -- This is the source of truth for upstream HLC ordering;
    -- if the gateway was offline for hours, `buffered_at`
    -- predates `forwarded_at` by that interval.
    buffered_at     TIMESTAMPTZ NOT NULL,
    -- Wall-time at the server when this row was inserted.
    -- Defaults to now() so the column is automatically
    -- populated; the gap between this and `buffered_at`
    -- measures buffer dwell time.
    forwarded_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- HLC for sync ordering. Same wire format as other
    -- replicated entities — `<wall_ms>.<logical>.<node>`.
    hlc             TEXT NOT NULL
);

-- "What did this gateway buffer in the last N hours" — the
-- workhorse query for the gateway forensics page. DESC on
-- buffered_at because that's the operator's mental model
-- (the newest backlog entry is the freshest data).
CREATE INDEX idx_gateway_samples_gateway_buffered
    ON gateway_samples(tenant_id, gateway_id, buffered_at DESC);

-- "What did we receive from this topic in the last hour" — the
-- cross-gateway view for an analyst tracking a single upstream
-- topic that multiple gateways may forward to.
CREATE INDEX idx_gateway_samples_topic_buffered
    ON gateway_samples(tenant_id, topic, buffered_at DESC);

ALTER TABLE gateway_samples ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_select_gateway_samples
    ON gateway_samples
    FOR SELECT USING (tenant_id = current_tenant());

CREATE POLICY scoped_insert_gateway_samples
    ON gateway_samples
    FOR INSERT WITH CHECK (tenant_id = current_tenant());

-- Audit-grade telemetry: no update, no delete after insert. The
-- "we lost data during the outage" question is answerable in
-- perpetuity from this ledger.
REVOKE UPDATE, DELETE ON gateway_samples FROM authenticated;
