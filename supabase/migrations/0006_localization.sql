-- Multi-Lingual & Multi-Currency Native — currencies, FX, tax rules,
-- per-tenant locale preferences.
--
-- Pairs with `crates/aether-money` and `packages/i18n`. FX snapshots are
-- stored in a TimescaleDB hypertable so historical transactions can be
-- reproduced even after rates change.

CREATE TABLE currencies (
    code         CHAR(3) PRIMARY KEY,
    name         TEXT NOT NULL,
    minor_units  SMALLINT NOT NULL,
    symbol       TEXT
);

INSERT INTO currencies (code, name, minor_units, symbol) VALUES
    ('USD', 'United States dollar', 2, '$'),
    ('EUR', 'Euro',                  2, '€'),
    ('GBP', 'Pound sterling',        2, '£'),
    ('JPY', 'Japanese yen',          0, '¥'),
    ('CNY', 'Chinese yuan',          2, '¥'),
    ('IDR', 'Indonesian rupiah',     0, 'Rp'),
    ('SGD', 'Singapore dollar',      2, 'S$'),
    ('AUD', 'Australian dollar',     2, 'A$'),
    ('INR', 'Indian rupee',          2, '₹'),
    ('BRL', 'Brazilian real',        2, 'R$')
ON CONFLICT DO NOTHING;

CREATE TABLE exchange_rates (
    ts          TIMESTAMPTZ NOT NULL,
    from_code   CHAR(3) NOT NULL REFERENCES currencies(code) ON DELETE CASCADE,
    to_code     CHAR(3) NOT NULL REFERENCES currencies(code) ON DELETE CASCADE,
    /* Stored as bp1e4 — multiplier × 1e8 — to avoid float drift. */
    bp1e4       BIGINT NOT NULL,
    source      TEXT NOT NULL,
    PRIMARY KEY (from_code, to_code, ts)
);
SELECT create_hypertable('exchange_rates', 'ts',
    chunk_time_interval => INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('exchange_rates', INTERVAL '5 years', if_not_exists => TRUE);

CREATE TABLE tax_rules (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country       CHAR(2) NOT NULL,
    subdivision   TEXT,
    scope         TEXT NOT NULL CHECK (scope IN ('value-added', 'sales', 'withholding', 'excise')),
    rate_bps      INTEGER NOT NULL,
    valid_from    TIMESTAMPTZ NOT NULL,
    valid_until   TIMESTAMPTZ,
    display       TEXT NOT NULL,
    notes         TEXT
);
CREATE INDEX idx_tax_rules_country ON tax_rules(country, valid_from DESC);

INSERT INTO tax_rules (country, subdivision, scope, rate_bps, valid_from, display, notes) VALUES
    ('ID', NULL,    'value-added', 1100, '2022-04-01', 'PPN 11%',     'Indonesia VAT'),
    ('DE', NULL,    'value-added', 1900, '2007-01-01', 'MwSt. 19%',   'Germany standard VAT'),
    ('GB', NULL,    'value-added', 2000, '2011-01-04', 'VAT 20%',     'UK standard rate'),
    ('AU', NULL,    'value-added', 1000, '2000-07-01', 'GST 10%',     'Australia GST'),
    ('JP', NULL,    'value-added', 1000, '2019-10-01', '消費税 10%',  'Japan consumption tax'),
    ('SG', NULL,    'value-added',  900, '2024-01-01', 'GST 9%',      'Singapore GST'),
    ('US', 'CA',    'sales',        725, '2017-01-01', 'CA sales tax 7.25%', NULL),
    ('US', 'NY',    'sales',        400, '2024-01-01', 'NY state sales 4%',  NULL)
ON CONFLICT DO NOTHING;

CREATE TABLE tenant_locale (
    tenant_id        UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    locale_tag       TEXT NOT NULL DEFAULT 'en-US',
    primary_currency CHAR(3) NOT NULL DEFAULT 'USD' REFERENCES currencies(code),
    timezone         TEXT NOT NULL DEFAULT 'UTC'
);
ALTER TABLE tenant_locale ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_tenant_locale ON tenant_locale
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_update_tenant_locale ON tenant_locale
    FOR UPDATE USING (tenant_id = current_tenant())
    WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'tenant:configure')
    );
