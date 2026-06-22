-- 0022_temperature_readings.sql
-- Industry-vertical capability table for the cold-chain-monitor flag (see
-- packages/industry-catalog, granted to food-and-beverage + pharmaceuticals).
-- Stores raw temperature samples per sensor. Tenant-scoped via the standard
-- current_tenant() / has_permission() pattern. Plain Postgres for now — when
-- migration 0002 (TimescaleDB) lands we can promote this to a hypertable
-- without changing the API surface.

CREATE TABLE IF NOT EXISTS temperature_readings (
    id            BIGSERIAL PRIMARY KEY,
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    sensor_id     TEXT NOT NULL,
    sensor_label  TEXT,
    taken_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    celsius       NUMERIC(7, 2) NOT NULL,
    /** Target range for the sensor (null means "any"). */
    low_c         NUMERIC(7, 2),
    high_c        NUMERIC(7, 2),
    hlc           TEXT NOT NULL DEFAULT '0.0.seed'
);

CREATE INDEX IF NOT EXISTS idx_temp_readings_tenant_sensor_time
    ON temperature_readings (tenant_id, sensor_id, taken_at DESC);

ALTER TABLE temperature_readings ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS temp_readings_tenant_read ON temperature_readings;
CREATE POLICY temp_readings_tenant_read ON temperature_readings
    FOR SELECT TO authenticated
    USING (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'inventory:read')
    );

-- Writes go through gateway-samples / hardware integrations only; clients have
-- no direct INSERT. (Seed and demo data come via service role.)
REVOKE INSERT, UPDATE, DELETE ON temperature_readings FROM anon, authenticated;
