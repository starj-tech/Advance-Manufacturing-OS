-- Global Supply Chain Hedging — commodities + price history + recommendations.
--
-- Pairs with `crates/aether-hedging`. Prices are stored in a TimescaleDB
-- hypertable so the moving-average heuristic and (future) ML forecasts
-- can scan months of data efficiently.

CREATE TABLE commodities (
    ticker      TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    unit        TEXT NOT NULL,
    description TEXT
);

INSERT INTO commodities (ticker, name, unit, description) VALUES
    ('HRC',  'Steel hot-rolled coil', 'USD/MT',     'CME HRC futures'),
    ('ALI',  'Aluminum',              'USD/MT',     'LME 3-month'),
    ('HG',   'Copper',                'USD/MT',     'COMEX copper'),
    ('PP',   'Polypropylene',         'USD/MT',     'CFR China resin'),
    ('BZ',   'Brent crude',           'USD/bbl',    'ICE Brent front-month'),
    ('NG',   'Natural gas',           'USD/MMBtu',  'NYMEX Henry Hub'),
    ('LITH', 'Lithium carbonate',     'USD/MT',     'CIF Asia battery-grade'),
    ('CO',   'Cobalt cathode',        'USD/MT',     'LME cobalt')
ON CONFLICT DO NOTHING;

CREATE TABLE commodity_prices (
    ts          TIMESTAMPTZ NOT NULL,
    ticker      TEXT NOT NULL REFERENCES commodities(ticker) ON DELETE CASCADE,
    price       DOUBLE PRECISION NOT NULL,
    usd_price   DOUBLE PRECISION NOT NULL,
    source      TEXT NOT NULL,
    PRIMARY KEY (ticker, ts)
);
SELECT create_hypertable('commodity_prices', 'ts',
    chunk_time_interval => INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('commodity_prices', INTERVAL '5 years', if_not_exists => TRUE);

CREATE MATERIALIZED VIEW commodity_prices_daily
WITH (timescaledb.continuous) AS
SELECT
    ticker,
    time_bucket('1 day', ts) AS day,
    avg(usd_price) AS avg_usd,
    min(usd_price) AS min_usd,
    max(usd_price) AS max_usd,
    last(usd_price, ts) AS close_usd
FROM commodity_prices
GROUP BY 1, 2
WITH NO DATA;
SELECT add_continuous_aggregate_policy(
    'commodity_prices_daily',
    start_offset => INTERVAL '1 year',
    end_offset   => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

CREATE TABLE material_commodity_map (
    tenant_id    UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    material_sku TEXT NOT NULL,
    ticker       TEXT NOT NULL REFERENCES commodities(ticker),
    weight       NUMERIC(4, 3) NOT NULL CHECK (weight >= 0 AND weight <= 1),
    PRIMARY KEY (tenant_id, material_sku, ticker)
);
ALTER TABLE material_commodity_map ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_material_commodity ON material_commodity_map
    FOR SELECT USING (tenant_id = current_tenant());

CREATE TABLE hedge_recommendations (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id              UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    ticker                 TEXT NOT NULL REFERENCES commodities(ticker),
    generated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    spot_usd               DOUBLE PRECISION NOT NULL,
    sma_usd                DOUBLE PRECISION NOT NULL,
    action                 TEXT NOT NULL CHECK (action IN ('buy-now', 'wait', 'hold')),
    cover_months           INTEGER,
    estimated_savings_pct  DOUBLE PRECISION,
    rationale              TEXT,
    /* When set, the recommendation has been accepted/dismissed by a user. */
    decided_by             UUID,
    decided_at             TIMESTAMPTZ,
    decision               TEXT CHECK (decision IN ('accepted', 'dismissed'))
);
CREATE INDEX idx_hedge_recommendations_tenant ON hedge_recommendations(tenant_id, generated_at DESC);
ALTER TABLE hedge_recommendations ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_hedge_recommendations ON hedge_recommendations
    FOR SELECT USING (tenant_id = current_tenant());
