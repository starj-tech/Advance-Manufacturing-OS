-- Row-Level Security policies.
--
-- Authoritative gate for tenant isolation. Client-side checks are UX
-- only; the server NEVER trusts a tenant_id sent by the client without
-- matching `auth.jwt() -> 'app_metadata' ->> 'tenant_id'`.

CREATE OR REPLACE FUNCTION current_tenant() RETURNS UUID
LANGUAGE SQL STABLE AS $$
    SELECT (auth.jwt() -> 'app_metadata' ->> 'tenant_id')::uuid
$$;

CREATE OR REPLACE FUNCTION has_permission(
    p_user_id   UUID,
    p_tenant_id UUID,
    p_scope     TEXT
) RETURNS BOOLEAN
LANGUAGE plpgsql SECURITY DEFINER STABLE AS $$
DECLARE
    v_role TEXT;
BEGIN
    SELECT primary_role INTO v_role
    FROM tenant_users
    WHERE tenant_id = p_tenant_id AND user_id = p_user_id;

    IF v_role IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Wildcard for developers / superusers
    IF EXISTS (
        SELECT 1 FROM permissions
        WHERE tenant_id = p_tenant_id
          AND role_name = v_role
          AND scope = '*'
          AND granted = TRUE
    ) THEN
        RETURN TRUE;
    END IF;

    -- Exact scope match
    IF EXISTS (
        SELECT 1 FROM permissions
        WHERE tenant_id = p_tenant_id
          AND role_name = v_role
          AND scope = p_scope
          AND granted = TRUE
    ) THEN
        RETURN TRUE;
    END IF;

    -- Resource wildcard (e.g. work_orders:*)
    IF EXISTS (
        SELECT 1 FROM permissions
        WHERE tenant_id = p_tenant_id
          AND role_name = v_role
          AND scope = split_part(p_scope, ':', 1) || ':*'
          AND granted = TRUE
    ) THEN
        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$;

REVOKE ALL ON FUNCTION has_permission(UUID, UUID, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION has_permission(UUID, UUID, TEXT) TO authenticated;

-- ===== Enable RLS on tenant-scoped tables =====
ALTER TABLE tenants                    ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_users               ENABLE ROW LEVEL SECURITY;
ALTER TABLE roles                      ENABLE ROW LEVEL SECURITY;
ALTER TABLE permissions                ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log                  ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_modules             ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_trusted_publishers  ENABLE ROW LEVEL SECURITY;
ALTER TABLE machines                   ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_orders                ENABLE ROW LEVEL SECURITY;
ALTER TABLE materials                  ENABLE ROW LEVEL SECURITY;
ALTER TABLE boms                       ENABLE ROW LEVEL SECURITY;
ALTER TABLE sync_changes               ENABLE ROW LEVEL SECURITY;
ALTER TABLE sos_events                 ENABLE ROW LEVEL SECURITY;
ALTER TABLE geofences                  ENABLE ROW LEVEL SECURITY;
ALTER TABLE telemetry                  ENABLE ROW LEVEL SECURITY;

-- Generic tenant-isolation read policy. Applied per-table for clarity.
CREATE POLICY tenant_isolation_select_machines     ON machines      FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_work_orders  ON work_orders   FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_materials    ON materials     FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_boms         ON boms          FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_sync_changes ON sync_changes  FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_audit_log    ON audit_log     FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_sos_events   ON sos_events    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_geofences    ON geofences     FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_telemetry    ON telemetry     FOR SELECT USING (tenant_id = current_tenant());

-- Scope-required INSERT/UPDATE policies. UPDATE on inventory `qty_on_hand`
-- is intentionally NOT granted here — clients must use the
-- inventory_adjust(delta) RPC (PR #2) which enforces qty >= 0 atomic.
CREATE POLICY scoped_insert_work_orders ON work_orders
    FOR INSERT WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'work_orders:create')
    );

CREATE POLICY scoped_update_work_orders ON work_orders
    FOR UPDATE USING (tenant_id = current_tenant())
    WITH CHECK (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'work_orders:update')
    );

-- Audit log: insert allowed for all authenticated tenant members; no UPDATE/DELETE.
CREATE POLICY scoped_insert_audit ON audit_log
    FOR INSERT WITH CHECK (tenant_id = current_tenant());

REVOKE UPDATE, DELETE ON audit_log FROM authenticated;
