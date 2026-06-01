-- 0024_shop_floor_tasks.sql
-- Shop-floor task cards for the employee shell. Lightweight task queue
-- assigned to a specific operator (or unassigned pool). Status machine is
-- intentionally minimal: todo → doing → done. RLS scopes by tenant + the
-- current user when assigned_to is set; permission gate `tasks:read` covers
-- the "see all" case.

CREATE TABLE IF NOT EXISTS shop_floor_tasks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    instruction     TEXT,
    assigned_to     UUID,
    work_order_id   UUID REFERENCES work_orders(id) ON DELETE SET NULL,
    machine_id      UUID REFERENCES machines(id) ON DELETE SET NULL,
    priority        TEXT NOT NULL DEFAULT 'normal'
                    CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    status          TEXT NOT NULL DEFAULT 'todo'
                    CHECK (status IN ('todo', 'doing', 'done')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    due_at          TIMESTAMPTZ,
    hlc             TEXT NOT NULL DEFAULT '0.0.seed'
);

CREATE INDEX IF NOT EXISTS idx_tasks_tenant_status_due
    ON shop_floor_tasks (tenant_id, status, due_at NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned
    ON shop_floor_tasks (tenant_id, assigned_to);

ALTER TABLE shop_floor_tasks ENABLE ROW LEVEL SECURITY;

-- Operators see everything for their tenant that they're allowed to read
-- (tasks:read covers operator + manager). Manager-only views can layer on
-- additional filters client-side.
DROP POLICY IF EXISTS tasks_tenant_read ON shop_floor_tasks;
CREATE POLICY tasks_tenant_read ON shop_floor_tasks
    FOR SELECT TO authenticated
    USING (
        tenant_id = current_tenant()
        AND has_permission(auth.uid(), tenant_id, 'tasks:read')
    );

-- An operator can claim/complete their own task: UPDATE allowed only when the
-- row's assigned_to is the caller (or NULL = unassigned pool) and the caller
-- has tasks:complete.
DROP POLICY IF EXISTS tasks_self_update ON shop_floor_tasks;
CREATE POLICY tasks_self_update ON shop_floor_tasks
    FOR UPDATE TO authenticated
    USING (
        tenant_id = current_tenant()
        AND (assigned_to = auth.uid() OR assigned_to IS NULL)
        AND has_permission(auth.uid(), tenant_id, 'tasks:complete')
    )
    WITH CHECK (
        tenant_id = current_tenant()
        AND (assigned_to = auth.uid() OR assigned_to IS NULL)
    );

-- Inserts are admin-only for now (manager UI lands in a later PR); deny on
-- the client side. Service role bypasses RLS.
REVOKE INSERT, DELETE ON shop_floor_tasks FROM anon, authenticated;
