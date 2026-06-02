-- 0026_gateway_samples.sql
-- Telemetry ingest target for the on-prem gateway (gateways/aether-gateway).
-- One row per sample emitted by a driver's poll() — tagged by machine + tag
-- name. Heavily indexed by (tenant, machine, taken_at) because that's the
-- only access pattern (latest N per machine).
--
-- Writes come from the gateway running as service-role, so RLS only needs a
-- SELECT policy for clients. INSERT/UPDATE/DELETE remain locked to clients
-- to keep the wire-protocol asymmetric.

CREATE TABLE IF NOT EXISTS gateway_samples (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    machine_id  UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    value       JSONB,
    taken_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    gateway_id  TEXT
);

CREATE INDEX IF NOT EXISTS idx_gateway_samples_tenant_machine_time
    ON gateway_samples (tenant_id, machine_id, taken_at DESC);
CREATE INDEX IF NOT EXISTS idx_gateway_samples_tenant_time
    ON gateway_samples (tenant_id, taken_at DESC);

ALTER TABLE gateway_samples ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS gateway_samples_tenant_read ON gateway_samples;
CREATE POLICY gateway_samples_tenant_read ON gateway_samples
    FOR SELECT TO authenticated
    USING (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'devices:read')
    );

REVOKE INSERT, UPDATE, DELETE ON gateway_samples FROM anon, authenticated;

-- Enable Realtime CDC for the tables Main / IT subscribe to in V124. Each
-- ADD TABLE runs in its own DO block so a duplicate-add doesn't abort the
-- migration if Supabase already auto-included the table.
DO $$ BEGIN
    BEGIN
        ALTER PUBLICATION supabase_realtime ADD TABLE gateway_samples;
    EXCEPTION WHEN OTHERS THEN
        NULL;
    END;
END $$;

DO $$ BEGIN
    BEGIN
        ALTER PUBLICATION supabase_realtime ADD TABLE temperature_readings;
    EXCEPTION WHEN OTHERS THEN
        NULL;
    END;
END $$;

DO $$ BEGIN
    BEGIN
        ALTER PUBLICATION supabase_realtime ADD TABLE audit_log;
    EXCEPTION WHEN OTHERS THEN
        NULL;
    END;
END $$;
