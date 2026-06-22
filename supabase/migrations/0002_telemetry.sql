-- TimescaleDB-backed telemetry hypertable.
--
-- If TimescaleDB is unavailable in your Supabase tier, drop the
-- create_hypertable() call — the table will still function as a regular
-- partitioned table with manual range pruning.

CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE telemetry (
    ts          TIMESTAMPTZ NOT NULL,
    tenant_id   UUID NOT NULL,
    machine_id  UUID NOT NULL,
    tag         TEXT NOT NULL,
    value       DOUBLE PRECISION,
    quality     SMALLINT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, machine_id, tag, ts)
);

SELECT create_hypertable('telemetry', 'ts', chunk_time_interval => INTERVAL '1 day', if_not_exists => TRUE);

-- 90-day retention.
SELECT add_retention_policy('telemetry', INTERVAL '90 days', if_not_exists => TRUE);

-- 1-minute downsampled rollup, refreshed continuously.
CREATE MATERIALIZED VIEW telemetry_1m
WITH (timescaledb.continuous) AS
SELECT
    tenant_id,
    machine_id,
    tag,
    time_bucket('1 minute', ts) AS bucket,
    avg(value)   AS avg_v,
    min(value)   AS min_v,
    max(value)   AS max_v,
    count(*)     AS n
FROM telemetry
GROUP BY 1, 2, 3, 4
WITH NO DATA;

SELECT add_continuous_aggregate_policy(
    'telemetry_1m',
    start_offset => INTERVAL '7 days',
    end_offset   => INTERVAL '1 minute',
    schedule_interval => INTERVAL '5 minutes',
    if_not_exists => TRUE
);
