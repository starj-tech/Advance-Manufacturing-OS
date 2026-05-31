-- 0020_work_order_advance.sql
-- Atomic, permission-gated work-order state transitions. Like 0019's
-- inventory_adjust: clients have no direct UPDATE on work_orders.status (see
-- 0003) and must go through this SECURITY DEFINER RPC, which validates the
-- transition, optionally bumps qty_done, refuses out-of-band edits via an
-- expected-HLC check, and emits an audit_log row.

CREATE OR REPLACE FUNCTION work_order_advance(
    p_id            UUID,
    p_to            TEXT,
    p_qty_delta     NUMERIC DEFAULT 0,
    p_expected_hlc  TEXT DEFAULT NULL,
    p_reason        TEXT DEFAULT NULL
) RETURNS TABLE (status TEXT, qty_done NUMERIC, hlc TEXT)
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = public, auth
AS $$
DECLARE
    v_tenant UUID := current_tenant();
    v_from   TEXT;
    v_hlc    TEXT;
    v_qty    NUMERIC;
    v_planned NUMERIC;
    v_new_hlc TEXT;
    v_new_qty NUMERIC;
BEGIN
    IF v_tenant IS NULL THEN
        RAISE EXCEPTION 'no tenant context';
    END IF;
    IF NOT has_permission(auth.uid(), v_tenant, 'work_orders:update') THEN
        RAISE EXCEPTION 'forbidden: work_orders:update required';
    END IF;
    IF p_to NOT IN ('draft', 'released', 'running', 'paused', 'completed', 'canceled') THEN
        RAISE EXCEPTION 'invalid target status: %', p_to;
    END IF;

    SELECT status, hlc, qty_done, qty_planned
      INTO v_from, v_hlc, v_qty, v_planned
      FROM work_orders
     WHERE id = p_id AND tenant_id = v_tenant
       FOR UPDATE;

    IF v_from IS NULL THEN
        RAISE EXCEPTION 'work order not found';
    END IF;
    IF p_expected_hlc IS NOT NULL AND p_expected_hlc <> v_hlc THEN
        RAISE EXCEPTION 'stale hlc: expected % but stored %', p_expected_hlc, v_hlc;
    END IF;

    -- State-machine: explicit allow-list of transitions.
    IF NOT (
        (v_from = 'draft'     AND p_to IN ('released', 'canceled')) OR
        (v_from = 'released'  AND p_to IN ('running', 'canceled')) OR
        (v_from = 'running'   AND p_to IN ('paused', 'completed', 'canceled')) OR
        (v_from = 'paused'    AND p_to IN ('running', 'canceled'))
    ) THEN
        RAISE EXCEPTION 'illegal transition: % -> %', v_from, p_to;
    END IF;

    v_new_qty := v_qty + COALESCE(p_qty_delta, 0);
    IF v_new_qty < 0 THEN
        RAISE EXCEPTION 'qty_done would go negative';
    END IF;
    IF p_to = 'completed' AND v_new_qty < v_planned THEN
        -- Auto-top-up qty_done to qty_planned on completion. Operators can
        -- still pre-bump partial quantities via p_qty_delta before completing.
        v_new_qty := v_planned;
    END IF;

    v_new_hlc := extract(epoch from now())::text || '.0.rpc';

    UPDATE work_orders
       SET status = p_to,
           qty_done = v_new_qty,
           hlc = v_new_hlc,
           parent_hlc = v_hlc
     WHERE id = p_id AND tenant_id = v_tenant;

    INSERT INTO audit_log (tenant_id, actor_id, action, resource, resource_id, metadata, hlc)
    VALUES (
        v_tenant, auth.uid(), 'work_order_advance', 'work_orders', p_id::text,
        jsonb_build_object(
            'from', v_from, 'to', p_to,
            'qty_delta', COALESCE(p_qty_delta, 0),
            'qty_done', v_new_qty,
            'reason', p_reason
        ),
        v_new_hlc
    );

    RETURN QUERY SELECT p_to::TEXT, v_new_qty, v_new_hlc;
END;
$$;

REVOKE ALL ON FUNCTION work_order_advance(UUID, TEXT, NUMERIC, TEXT, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION work_order_advance(UUID, TEXT, NUMERIC, TEXT, TEXT) TO authenticated;
