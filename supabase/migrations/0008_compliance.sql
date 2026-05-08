-- Instant Compliance — per-tenant scope + signed report ledger.
--
-- Standards + control points are defined in code (`crates/aether-compliance`)
-- so the catalog ships with the binary and stays in lock-step with the
-- probes. This schema only stores per-tenant scope and signed report
-- attestations.

CREATE TABLE tenant_compliance_scope (
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    standard_slug  TEXT NOT NULL,
    enabled        BOOLEAN NOT NULL DEFAULT TRUE,
    enrolled_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    enrolled_by    UUID,
    PRIMARY KEY (tenant_id, standard_slug)
);
ALTER TABLE tenant_compliance_scope ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_compliance_scope
    ON tenant_compliance_scope
    FOR SELECT USING (tenant_id = current_tenant());

CREATE TABLE compliance_reports (
    id                       UUID PRIMARY KEY,
    tenant_id                UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    standard_slug            TEXT NOT NULL,
    generated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    /* compliant | needs-review | non-compliant */
    status                   TEXT NOT NULL,
    controls_total           INTEGER NOT NULL,
    controls_passed          INTEGER NOT NULL,
    controls_failed          INTEGER NOT NULL,
    controls_needs_review    INTEGER NOT NULL,
    /* JSONB array of { control_point_id, verdict, evidence } */
    controls                 JSONB NOT NULL,
    /* Cloud Edge Function signature over the canonicalized report payload. */
    attestation_signature    BYTEA,
    attestation_key_id       TEXT
);
CREATE INDEX idx_compliance_tenant_generated
    ON compliance_reports(tenant_id, standard_slug, generated_at DESC);
ALTER TABLE compliance_reports ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_compliance_reports
    ON compliance_reports
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_insert_compliance_reports
    ON compliance_reports
    FOR INSERT WITH CHECK (tenant_id = current_tenant());
REVOKE UPDATE, DELETE ON compliance_reports FROM authenticated;

CREATE TABLE compliance_evidence (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    report_id     UUID NOT NULL REFERENCES compliance_reports(id) ON DELETE CASCADE,
    control_point TEXT NOT NULL,
    /* Where the probe pulled evidence from: 'audit_log', 'sync_changes',
       'certifications', 'storage://bucket/path', etc. */
    source        TEXT NOT NULL,
    /* Hash chain so evidence can be re-verified without exposing the
       referenced row to anyone outside the tenant. */
    evidence_hash BYTEA NOT NULL
);
CREATE INDEX idx_compliance_evidence_report ON compliance_evidence(report_id);
ALTER TABLE compliance_evidence ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_compliance_evidence
    ON compliance_evidence
    FOR SELECT USING (tenant_id = current_tenant());
