-- Universal Module — per-tenant industry vertical + capability overrides.
--
-- The 21 industries themselves live in code (`crates/aether-industry`);
-- this schema only stores per-tenant assignment + overrides. Onboarding
-- writes a single row; the desktop client resolves the full profile by
-- calling `aether_industry::profile_for(industry)`.

CREATE TABLE tenant_industry (
    tenant_id     UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    industry_slug TEXT NOT NULL,
    set_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    set_by        UUID
);
ALTER TABLE tenant_industry ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_industry ON tenant_industry
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_update_tenant_industry ON tenant_industry
    FOR UPDATE USING (tenant_id = current_tenant())
    WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'tenant:configure')
    );

-- Per-tenant capability override. NULL `enabled` means "use the
-- profile default"; explicit TRUE/FALSE forces the value.
CREATE TABLE tenant_capability_overrides (
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    capability_id  TEXT NOT NULL,
    enabled        BOOLEAN,
    note           TEXT,
    set_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    set_by         UUID,
    PRIMARY KEY (tenant_id, capability_id)
);
ALTER TABLE tenant_capability_overrides ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation_select_capability_overrides
    ON tenant_capability_overrides
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY scoped_upsert_capability_overrides
    ON tenant_capability_overrides
    FOR INSERT WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'tenant:configure')
    );
