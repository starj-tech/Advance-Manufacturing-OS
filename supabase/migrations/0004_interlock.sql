-- Smart Interlock Safety — certifications + machine permits + audit.
--
-- Pairs with `crates/aether-safety/src/interlock.rs`. Certifications are
-- per-tenant; machine permit policies live alongside the machine row.
-- Verdicts are written into `interlock_events` for forensics.

CREATE TABLE certifications (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL,
    code        TEXT NOT NULL,
    issued_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ,
    revoked     BOOLEAN NOT NULL DEFAULT FALSE,
    issued_by   UUID,
    -- Free-form proof reference (PDF in Storage, scan id, etc).
    evidence    JSONB,
    UNIQUE (tenant_id, user_id, code)
);
CREATE INDEX idx_certifications_user ON certifications(tenant_id, user_id);
ALTER TABLE certifications ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_certifications ON certifications
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_insert_certifications ON certifications
    FOR INSERT WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'certifications:issue')
    );

CREATE TABLE machine_required_certs (
    tenant_id   UUID NOT NULL,
    machine_id  UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    cert_code   TEXT NOT NULL,
    PRIMARY KEY (tenant_id, machine_id, cert_code)
);
ALTER TABLE machine_required_certs ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_required_certs ON machine_required_certs
    FOR SELECT USING (tenant_id = current_tenant());

CREATE TABLE machine_permits (
    machine_id     UUID PRIMARY KEY REFERENCES machines(id) ON DELETE CASCADE,
    tenant_id      UUID NOT NULL,
    /* Protocol target for the safety relay coil/node, e.g.
       opcua://plc1/ns=2;s=Permit  or  modbus://10.0.0.5/coils/0x0010 */
    permit_target  TEXT NOT NULL,
    last_state     BOOLEAN,
    last_set_at    TIMESTAMPTZ,
    last_set_by    UUID
);
ALTER TABLE machine_permits ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_permits ON machine_permits
    FOR SELECT USING (tenant_id = current_tenant());

CREATE TABLE interlock_events (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL,
    machine_id   UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    /* Verdict::Allow | DenyMissingCert | DenyExpiredCert | DenyLockout | DenyMachineFault */
    verdict      TEXT NOT NULL,
    detail       JSONB,
    /* Whether the safety relay write actually succeeded on the wire. */
    permit_set   BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_interlock_events_machine_at ON interlock_events(machine_id, at DESC);
ALTER TABLE interlock_events ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_interlock_events ON interlock_events
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_insert_interlock_events ON interlock_events
    FOR INSERT WITH CHECK (tenant_id = current_tenant());
REVOKE UPDATE, DELETE ON interlock_events FROM authenticated;
