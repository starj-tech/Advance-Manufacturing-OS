-- Scan events — append-only ledger for typed scan payloads
-- produced by `aether_autoid::Scanner` + (optionally) processed
-- by `aether_autoid::ScanPipeline`.
--
-- Distinct from `actuator_commands` (V10): that table audits
-- per-command rows (the Scan dispatch with its permit + outcome
-- metadata). This one ledgers per-payload data (the decoded
-- bytes + symbology class + handler verdict if a pipeline was
-- wired). An FSMA 204 analyst's question "every GS1-128 lot
-- scanned in the last hour" filters this table; "every Scan
-- command issued" filters actuator_commands.
--
-- The flow: scanner reads → `Scan` dispatch (audited in
-- actuator_commands) → ScanPipeline handles (writes ScanLedger
-- entry, optional) → `scan_event::encode_scan` produces an
-- outbox `Insert` → V10 outbox flushes upstream → server
-- applies row here.

CREATE TABLE scan_events (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- The scanner that produced the payload. Not a FK because
    -- scanner identity is owned by the runtime (`ScannerId(Uuid)`
    -- from aether-core); not every scanner has a Postgres row.
    scanner_id      UUID NOT NULL,
    -- Optional cross-reference to the machine the scan was
    -- associated with. NULL for unbound mobile scanners and
    -- workflow steps that don't tie scans to a machine.
    machine_id      UUID REFERENCES machines(id) ON DELETE SET NULL,
    -- Optional operator user_id for badge-bound sessions. NULL
    -- for fixed industrial readers in continuous-mode that don't
    -- bind to a user.
    user_id         UUID,

    -- Kebab-case symbology / RF protocol slug. Pinned in
    -- `aether_autoid::ScanClass::slug()` — 13 variants spanning
    -- 1D / 2D / RFID / NFC. Renaming a variant in code is a
    -- schema migration.
    scan_class      TEXT NOT NULL
                    CHECK (scan_class IN (
                        'code-128', 'ean-13', 'gs1-128', 'itf-14',
                        'qr-code', 'data-matrix', 'aztec', 'pdf-417',
                        'rfid-iso-14443-a', 'rfid-iso-14443-b',
                        'rfid-iso-15693', 'uhf', 'nfc-type-a'
                    )),

    -- Raw decoded payload bytes. UTF-8 text for symbology
    -- codes (1D/2D), binary UID + memory for RFID/NFC. Capped
    -- at 4 KiB — scan payloads in pilots top out near 200
    -- bytes (longest observed: a multi-AI GS1-128 lot string).
    -- Larger payloads (NFC NDEF dumps over 4 KiB) take the V4
    -- sealed-frame path.
    payload         BYTEA NOT NULL
                    CHECK (octet_length(payload) <= 4096),

    -- Vendor confidence 0..=1 where the reader exposes one
    -- (Cognex Dataman, Honeywell N6603). NULL for RFID/NFC
    -- which is binary read/no-read.
    confidence      REAL CHECK (confidence IS NULL
                               OR (confidence >= 0.0 AND confidence <= 1.0)),

    -- V12 ScanPipeline handler verdict — when a pipeline
    -- processed the scan. NULL for scans surfaced directly
    -- (no handler routing).
    handler_name    TEXT,
    handler_action  TEXT
                    CHECK (handler_action IS NULL
                          OR handler_action IN ('acknowledge', 'reject', 'trigger')),
    handler_summary TEXT,

    -- Wall-time at the scanner when the read completed. Source
    -- of truth for ordering when the scanner buffered the scan
    -- before the orchestrator drained.
    scanned_at      TIMESTAMPTZ NOT NULL,
    -- Wall-time at the server when this row was inserted.
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- HLC for sync ordering.
    hlc             TEXT NOT NULL
);

-- "What did this scanner read in the last hour" — workhorse
-- for the scanner-binding troubleshooting page.
CREATE INDEX idx_scan_events_scanner_scanned
    ON scan_events(tenant_id, scanner_id, scanned_at DESC);

-- "Every GS1-128 lot scanned this shift" — the FSMA 204 +
-- ISO 22000 compliance probe filter.
CREATE INDEX idx_scan_events_class_scanned
    ON scan_events(tenant_id, scan_class, scanned_at DESC);

-- "Scans tied to this machine" — partial index keeps it tiny
-- since many scans aren't machine-bound.
CREATE INDEX idx_scan_events_machine_scanned
    ON scan_events(tenant_id, machine_id, scanned_at DESC)
    WHERE machine_id IS NOT NULL;

ALTER TABLE scan_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_select_scan_events
    ON scan_events
    FOR SELECT USING (tenant_id = current_tenant());

CREATE POLICY scoped_insert_scan_events
    ON scan_events
    FOR INSERT WITH CHECK (tenant_id = current_tenant());

-- Audit-grade: every scan stays forever-queryable. FSMA 204
-- mandates lot-level traceability for 2 years; ISO 22000 for
-- the product's shelf life.
REVOKE UPDATE, DELETE ON scan_events FROM authenticated;
