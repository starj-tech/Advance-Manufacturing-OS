-- AETHER-OS local SQLite — initial schema.
-- Mirrors supabase/migrations/0001_core.sql but adapted for SQLite:
--   * UUID stored as TEXT
--   * JSONB stored as TEXT (validated at app layer)
--   * NUMERIC stored as TEXT to preserve precision
--   * TIMESTAMPTZ stored as ISO-8601 TEXT
--
-- The schema-sync test in CI verifies these mirror correctly per entity.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id                       TEXT PRIMARY KEY,
    email_ciphertext         BLOB,
    email_blind_idx          BLOB,
    display_name_ciphertext  BLOB,
    status                   TEXT NOT NULL CHECK (status IN ('active','suspended','offboarded')),
    created_at               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS roles (
    role_name   TEXT PRIMARY KEY,
    description TEXT
);

CREATE TABLE IF NOT EXISTS permissions (
    role_name TEXT NOT NULL,
    scope     TEXT NOT NULL,
    granted   INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (role_name, scope),
    FOREIGN KEY (role_name) REFERENCES roles(role_name) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS machines (
    id               TEXT PRIMARY KEY,
    code             TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    protocol_binding TEXT,
    status           TEXT,
    last_heartbeat   TEXT,
    hlc              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS work_orders (
    id           TEXT PRIMARY KEY,
    code         TEXT NOT NULL UNIQUE,
    machine_id   TEXT REFERENCES machines(id),
    status       TEXT NOT NULL CHECK (status IN ('draft','released','running','paused','completed','canceled')),
    qty_planned  TEXT NOT NULL DEFAULT '0',
    qty_done     TEXT NOT NULL DEFAULT '0',
    due_at       TEXT,
    created_by   TEXT,
    hlc          TEXT NOT NULL,
    parent_hlc   TEXT
);

CREATE TABLE IF NOT EXISTS materials (
    id               TEXT PRIMARY KEY,
    sku              TEXT NOT NULL UNIQUE,
    name_ciphertext  BLOB,
    name_blind_idx   BLOB,
    uom              TEXT,
    qty_on_hand      TEXT NOT NULL DEFAULT '0',
    qty_reserved     TEXT NOT NULL DEFAULT '0',
    hlc              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS boms (
    id                   TEXT PRIMARY KEY,
    product_sku          TEXT NOT NULL UNIQUE,
    document_ciphertext  BLOB,
    document_hash        BLOB,
    hlc                  TEXT NOT NULL
);

-- Telemetry: append-only, kept rolling (24h raw, downsampled later).
CREATE TABLE IF NOT EXISTS telemetry (
    ts         TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    tag        TEXT NOT NULL,
    value      REAL NOT NULL,
    quality    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ts, machine_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_telemetry_machine_tag_ts
    ON telemetry(machine_id, tag, ts);

-- Sync infrastructure (client-side only).
CREATE TABLE IF NOT EXISTS sync_outbox (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    op_id       TEXT NOT NULL UNIQUE,
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    op          TEXT NOT NULL CHECK (op IN ('insert','update','delete','crdt_patch')),
    payload     BLOB,
    hlc_ts      TEXT NOT NULL,
    parent_hlc  TEXT,
    encrypted   INTEGER NOT NULL DEFAULT 0,
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT,
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_outbox_entity ON sync_outbox(entity, entity_id);

CREATE TABLE IF NOT EXISTS sync_metadata (
    entity            TEXT PRIMARY KEY,
    last_pulled_hlc   TEXT,
    last_pushed_hlc   TEXT
);

CREATE TABLE IF NOT EXISTS tombstones (
    entity     TEXT NOT NULL,
    entity_id  TEXT NOT NULL,
    deleted_at TEXT NOT NULL,
    PRIMARY KEY (entity, entity_id)
);

-- Audit log (immutable; append-only).
CREATE TABLE IF NOT EXISTS audit_log (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_id            TEXT,
    action              TEXT NOT NULL,
    resource            TEXT,
    resource_id         TEXT,
    metadata            TEXT,
    payload_ciphertext  BLOB,
    hlc                 TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- Modules registry mirror.
CREATE TABLE IF NOT EXISTS modules (
    id              TEXT PRIMARY KEY,
    publisher_id    TEXT NOT NULL,
    current_version TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('published','disabled','deprecated')),
    manifest        TEXT,
    signature       BLOB NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS sos_events (
    id                   TEXT PRIMARY KEY,
    user_id              TEXT NOT NULL,
    triggered_at         TEXT NOT NULL,
    location_lat         REAL,
    location_lng         REAL,
    location_accuracy_m  INTEGER,
    status               TEXT NOT NULL CHECK (status IN ('active','acknowledged','resolved','false-alarm')),
    acknowledged_by      TEXT,
    acknowledged_at      TEXT,
    note                 TEXT
);
