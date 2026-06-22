-- 0025_machine_upsert.sql
-- Permission-gated machine binding RPC. RLS on machines (0003) only grants
-- SELECT; writes go through this SECURITY DEFINER function which validates
-- the protocol family, normalizes the JSONB binding, and audits the change.
--
-- Protocol allow-list is intentionally broad so a tenant can integrate
-- hardware "through any available pathway":
--   gateway-mediated:   opcua, mqtt, modbus-tcp, modbus-rtu, http-poll
--   browser-direct:     mqtt-ws, websocket, web-serial, web-usb,
--                       web-bluetooth, web-hid
--   misc:               manual, file-upload
-- New protocol slugs land here as the gateway/runtime gains support.

CREATE OR REPLACE FUNCTION machine_upsert(
    p_id        UUID,
    p_code      TEXT,
    p_name      TEXT,
    p_status    TEXT,
    p_protocol  TEXT,
    p_binding   JSONB DEFAULT '{}'::jsonb
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = public, auth
AS $$
DECLARE
    v_tenant UUID := current_tenant();
    v_id UUID;
    v_existing TEXT;
    v_action TEXT;
    v_hlc TEXT := extract(epoch from now())::text || '.0.rpc';
BEGIN
    IF v_tenant IS NULL THEN
        RAISE EXCEPTION 'no tenant context';
    END IF;
    IF NOT has_permission(auth.uid(), v_tenant, 'devices:write') THEN
        RAISE EXCEPTION 'forbidden: devices:write required';
    END IF;
    IF p_protocol NOT IN (
        'opcua', 'mqtt', 'mqtt-ws', 'modbus-tcp', 'modbus-rtu', 'http-poll',
        'websocket', 'web-serial', 'web-usb', 'web-bluetooth', 'web-hid',
        'manual', 'file-upload'
    ) THEN
        RAISE EXCEPTION 'invalid protocol: %', p_protocol;
    END IF;
    IF p_code IS NULL OR length(trim(p_code)) = 0 THEN
        RAISE EXCEPTION 'code required';
    END IF;
    IF p_name IS NULL OR length(trim(p_name)) = 0 THEN
        RAISE EXCEPTION 'name required';
    END IF;
    IF p_status NOT IN ('running', 'idle', 'paused', 'fault', 'maintenance', 'offline') THEN
        RAISE EXCEPTION 'invalid status: %', p_status;
    END IF;

    -- Merge binding JSON with the protocol slug so consumers have a single
    -- source of truth: protocol_binding.protocol == row.protocol_binding->>'protocol'.
    p_binding := COALESCE(p_binding, '{}'::jsonb) || jsonb_build_object('protocol', p_protocol);

    IF p_id IS NULL THEN
        INSERT INTO machines (tenant_id, code, name, status, protocol_binding, hlc)
        VALUES (v_tenant, p_code, p_name, p_status, p_binding, v_hlc)
        RETURNING id INTO v_id;
        v_action := 'machine_created';
    ELSE
        SELECT code INTO v_existing FROM machines
         WHERE id = p_id AND tenant_id = v_tenant
         FOR UPDATE;
        IF v_existing IS NULL THEN
            RAISE EXCEPTION 'machine not found';
        END IF;
        UPDATE machines
           SET code = p_code,
               name = p_name,
               status = p_status,
               protocol_binding = p_binding,
               hlc = v_hlc
         WHERE id = p_id AND tenant_id = v_tenant;
        v_id := p_id;
        v_action := 'machine_updated';
    END IF;

    INSERT INTO audit_log (tenant_id, actor_id, action, resource, resource_id, metadata, hlc)
    VALUES (
        v_tenant, auth.uid(), v_action, 'machines', v_id::text,
        jsonb_build_object('code', p_code, 'protocol', p_protocol, 'binding', p_binding),
        v_hlc
    );

    RETURN v_id;
END;
$$;

REVOKE ALL ON FUNCTION machine_upsert(UUID, TEXT, TEXT, TEXT, TEXT, JSONB) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION machine_upsert(UUID, TEXT, TEXT, TEXT, TEXT, JSONB) TO authenticated;
