-- 0023_lot_genealogy.sql
-- Industry-vertical capability table for the lot-genealogy flag (granted to
-- food-and-beverage, pharmaceuticals, etc.). Lots form a self-referential
-- tree: raw material → intermediate → finished. Tenant-scoped via the
-- standard current_tenant() / has_permission() pattern.

CREATE TABLE IF NOT EXISTS lots (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lot_code      TEXT NOT NULL,
    material_sku  TEXT,
    description   TEXT,
    qty           NUMERIC(18, 4),
    uom           TEXT,
    made_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ,
    parent_lot_id UUID REFERENCES lots(id) ON DELETE SET NULL,
    work_order_id UUID REFERENCES work_orders(id) ON DELETE SET NULL,
    hlc           TEXT NOT NULL DEFAULT '0.0.seed',
    UNIQUE (tenant_id, lot_code)
);

CREATE INDEX IF NOT EXISTS idx_lots_tenant_made
    ON lots (tenant_id, made_at DESC);
CREATE INDEX IF NOT EXISTS idx_lots_parent
    ON lots (parent_lot_id);

ALTER TABLE lots ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS lots_tenant_read ON lots;
CREATE POLICY lots_tenant_read ON lots
    FOR SELECT TO authenticated
    USING (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'inventory:read')
    );

-- Writes go through future production-order RPCs; clients have no direct
-- INSERT until that path lands. Seed via service role.
REVOKE INSERT, UPDATE, DELETE ON lots FROM anon, authenticated;
