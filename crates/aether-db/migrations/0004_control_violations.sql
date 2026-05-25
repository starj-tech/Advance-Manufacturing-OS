-- Generic per-control violation ledger.
--
-- Backs the compliance ViolationCountProbe: the count/violation family of
-- controls (non-conformances, open CAPAs, uncorrected out-of-spec events,
-- OHS incidents, overdue MDR filings, unencrypted-PII findings, …) is
-- satisfied when there are zero unresolved violations for the control.
-- One row per violation, keyed by the control point id it offends;
-- `resolved_at IS NULL` means still open. RFC3339 UTC TEXT timestamps,
-- matching the other evidence tables.
CREATE TABLE IF NOT EXISTS control_violations (
    id          TEXT PRIMARY KEY,
    control_id  TEXT NOT NULL,
    opened_at   TEXT NOT NULL,
    resolved_at TEXT,
    detail      TEXT
);
CREATE INDEX IF NOT EXISTS idx_violations_control_open
    ON control_violations(control_id, resolved_at);
