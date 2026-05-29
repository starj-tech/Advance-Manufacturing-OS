-- 0019_inventory_adjust.sql
-- Atomic, permission-gated inventory adjustment. RLS deliberately does NOT
-- grant UPDATE on materials.qty_on_hand (see 0003) — clients call this
-- SECURITY DEFINER RPC, which enforces qty >= 0 and the `inventory:adjust`
-- scope, and writes an audit_log row.

CREATE OR REPLACE FUNCTION inventory_adjust(
    p_material_id UUID,
    p_delta       NUMERIC,
    p_reason      TEXT DEFAULT NULL
) RETURNS NUMERIC
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = public, auth
AS $$
DECLARE
    v_tenant UUID := current_tenant();
    v_new    NUMERIC;
BEGIN
    IF v_tenant IS NULL THEN
        RAISE EXCEPTION 'no tenant context';
    END IF;
    IF NOT has_permission(auth.uid(), v_tenant, 'inventory:adjust') THEN
        RAISE EXCEPTION 'forbidden: inventory:adjust required';
    END IF;

    UPDATE materials
       SET qty_on_hand = qty_on_hand + p_delta
     WHERE id = p_material_id
       AND tenant_id = v_tenant
       AND qty_on_hand + p_delta >= 0
    RETURNING qty_on_hand INTO v_new;

    IF v_new IS NULL THEN
        RAISE EXCEPTION 'adjust rejected: material not found or would go negative';
    END IF;

    INSERT INTO audit_log (tenant_id, actor_id, action, resource, resource_id, metadata, hlc)
    VALUES (
        v_tenant, auth.uid(), 'inventory_adjust', 'materials', p_material_id::text,
        jsonb_build_object('delta', p_delta, 'reason', p_reason, 'new_qty', v_new),
        extract(epoch from now())::text || '.0.rpc'
    );

    RETURN v_new;
END;
$$;

REVOKE ALL ON FUNCTION inventory_adjust(UUID, NUMERIC, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION inventory_adjust(UUID, NUMERIC, TEXT) TO authenticated;
