-- AETHER-OS core schema.
--
-- Multi-tenant Postgres backbone. Companion SQLite mirror lives at
-- crates/aether-db/migrations/0001_init.sql; the schema-sync test in CI
-- verifies that round-tripping each entity preserves data.
--
-- Encrypted columns (e.g. *_ciphertext) hold ciphertext produced by the
-- aether-crypto crate on the client. Server NEVER has the master key.

-- Required extensions
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ===== Tenancy =====
CREATE TABLE tenants (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    region      TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id                       UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    email_ciphertext         BYTEA,
    email_blind_idx          BYTEA,
    display_name_ciphertext  BYTEA,
    status                   TEXT NOT NULL DEFAULT 'active'
                              CHECK (status IN ('active', 'suspended', 'offboarded')),
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tenant_users (
    tenant_id    UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    primary_role TEXT NOT NULL
                  CHECK (primary_role IN ('developer', 'executive', 'manager', 'employee')),
    joined_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);

CREATE TABLE roles (
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    role_name   TEXT NOT NULL,
    description TEXT,
    PRIMARY KEY (tenant_id, role_name)
);

CREATE TABLE permissions (
    tenant_id UUID NOT NULL,
    role_name TEXT NOT NULL,
    scope     TEXT NOT NULL,
    granted   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (tenant_id, role_name, scope),
    FOREIGN KEY (tenant_id, role_name)
        REFERENCES roles(tenant_id, role_name) ON DELETE CASCADE
);

-- ===== Audit log (immutable, append-only) =====
CREATE TABLE audit_log (
    id                  BIGSERIAL PRIMARY KEY,
    tenant_id           UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_id            UUID,
    action              TEXT NOT NULL,
    resource            TEXT,
    resource_id         TEXT,
    metadata            JSONB,
    payload_ciphertext  BYTEA,
    hlc                 TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_audit_tenant_created_at ON audit_log(tenant_id, created_at DESC);

-- ===== Module registry =====
CREATE TABLE modules (
    id                   TEXT PRIMARY KEY,                     -- com.vendor.module
    publisher_id         UUID NOT NULL,
    current_version      TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'published'
                          CHECK (status IN ('published', 'disabled', 'deprecated')),
    manifest_ciphertext  BYTEA,
    signature            BYTEA NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tenant_modules (
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    module_id      TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    pinned_version TEXT,
    policy         TEXT NOT NULL DEFAULT 'manual'
                    CHECK (policy IN ('auto', 'manual', 'pinned')),
    enabled        BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (tenant_id, module_id)
);

CREATE TABLE tenant_trusted_publishers (
    tenant_id        UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    publisher_key_id TEXT NOT NULL,
    public_key       BYTEA NOT NULL,
    trusted_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, publisher_key_id)
);

-- ===== Manufacturing core =====
CREATE TABLE machines (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code              TEXT NOT NULL,
    name              TEXT NOT NULL,
    protocol_binding  JSONB,
    status            TEXT,
    last_heartbeat    TIMESTAMPTZ,
    hlc               TEXT NOT NULL,
    UNIQUE (tenant_id, code)
);

CREATE TABLE work_orders (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code         TEXT NOT NULL,
    machine_id   UUID REFERENCES machines(id) ON DELETE SET NULL,
    status       TEXT NOT NULL
                  CHECK (status IN ('draft', 'released', 'running', 'paused', 'completed', 'canceled')),
    qty_planned  NUMERIC(18, 4) NOT NULL DEFAULT 0,
    qty_done     NUMERIC(18, 4) NOT NULL DEFAULT 0,
    due_at       TIMESTAMPTZ,
    created_by   UUID,
    hlc          TEXT NOT NULL,
    parent_hlc   TEXT,
    UNIQUE (tenant_id, code)
);
CREATE INDEX idx_work_orders_tenant_status ON work_orders(tenant_id, status);

CREATE TABLE materials (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    sku             TEXT NOT NULL,
    name_ciphertext BYTEA,
    name_blind_idx  BYTEA,
    uom             TEXT,
    qty_on_hand     NUMERIC(18, 4) NOT NULL DEFAULT 0,
    qty_reserved    NUMERIC(18, 4) NOT NULL DEFAULT 0,
    hlc             TEXT NOT NULL,
    UNIQUE (tenant_id, sku),
    CHECK (qty_on_hand >= 0)
);

CREATE TABLE boms (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    product_sku         TEXT NOT NULL,
    document_ciphertext BYTEA,    -- Automerge CRDT bytes, encrypted client-side
    document_hash       BYTEA,
    hlc                 TEXT NOT NULL,
    UNIQUE (tenant_id, product_sku)
);

-- ===== Sync infrastructure (server side) =====
CREATE TABLE sync_changes (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    op          TEXT NOT NULL CHECK (op IN ('insert', 'update', 'delete', 'crdt_patch')),
    payload     JSONB,
    hlc         TEXT NOT NULL,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_sync_changes_tenant_hlc ON sync_changes(tenant_id, hlc);

-- ===== Safety =====
CREATE TABLE sos_events (
    id                    UUID PRIMARY KEY,                -- UUIDv7 from client
    tenant_id             UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id               UUID NOT NULL,
    triggered_at          TIMESTAMPTZ NOT NULL,
    location_lat          NUMERIC(9, 6),
    location_lng          NUMERIC(9, 6),
    location_accuracy_m   INTEGER,
    status                TEXT NOT NULL DEFAULT 'active'
                           CHECK (status IN ('active', 'acknowledged', 'resolved', 'false-alarm')),
    acknowledged_by       UUID,
    acknowledged_at       TIMESTAMPTZ,
    note                  TEXT
);
CREATE INDEX idx_sos_tenant_active ON sos_events(tenant_id, status) WHERE status = 'active';

CREATE TABLE geofences (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    polygon_geojson     JSONB,
    allowed_bssids      TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    allowed_ble_beacons TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[]
);
