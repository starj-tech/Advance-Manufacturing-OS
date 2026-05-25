-- Compliance evidence tables.
--
-- These back the `aether-compliance` probes' EvidenceSource queries.
-- All timestamps are RFC3339 UTC TEXT (matching audit_log's convention)
-- so lexicographic comparison equals chronological comparison. Rows are
-- written by the operational layers (safety, crypto, sync); the
-- compliance crate only reads them.

-- INSERT-only ledger of attempted UPDATE/DELETE against audit_log. The
-- audit_log itself is append-only by policy; a row here means something
-- tried to rewrite history. Tamper detection reads "any row since X?".
CREATE TABLE IF NOT EXISTS audit_log_mutations (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    attempted_at TEXT NOT NULL,
    op           TEXT NOT NULL CHECK (op IN ('update','delete')),
    detail       TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_mut_at ON audit_log_mutations(attempted_at);

-- Security / safety incidents. `resolved_at IS NULL` means still open.
CREATE TABLE IF NOT EXISTS security_incidents (
    id          TEXT PRIMARY KEY,
    opened_at   TEXT NOT NULL,
    severity    TEXT NOT NULL CHECK (severity IN ('low','medium','high','critical')),
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_incidents_open ON security_incidents(resolved_at, opened_at);

-- Data-encryption-key rotation events. Newest row = current epoch.
CREATE TABLE IF NOT EXISTS key_rotation_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    rotated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_key_rotation_at ON key_rotation_events(rotated_at);

-- Completed periodic reviews, keyed by the control point they satisfy
-- (e.g. is-access-review, qms-internal-audit, qms-mgmt-review).
CREATE TABLE IF NOT EXISTS periodic_reviews (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    control_id  TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reviews_control_at ON periodic_reviews(control_id, reviewed_at);

-- Cold-chain temperature excursions (reading outside the safe band).
CREATE TABLE IF NOT EXISTS cold_chain_excursions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL,
    machine_id  TEXT,
    detail      TEXT
);
CREATE INDEX IF NOT EXISTS idx_cold_chain_at ON cold_chain_excursions(occurred_at);

-- Personal-data breaches. `notified_at IS NULL` = not yet notified.
CREATE TABLE IF NOT EXISTS data_breaches (
    id          TEXT PRIMARY KEY,
    detected_at TEXT NOT NULL,
    notified_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_breaches_notify ON data_breaches(notified_at, detected_at);

-- Data-subject access requests. `fulfilled_at IS NULL` = still open.
CREATE TABLE IF NOT EXISTS dsar_requests (
    id           TEXT PRIMARY KEY,
    requested_at TEXT NOT NULL,
    fulfilled_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_dsar_fulfil ON dsar_requests(fulfilled_at, requested_at);

-- Electronic signatures. `bound = 0` means the signature is not
-- cryptographically tied to its record (FDA 21 CFR 11.70 defect).
CREATE TABLE IF NOT EXISTS electronic_signatures (
    id         TEXT PRIMARY KEY,
    record_ref TEXT NOT NULL,
    bound      INTEGER NOT NULL DEFAULT 1 CHECK (bound IN (0, 1)),
    signed_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_esign_bound ON electronic_signatures(bound);
