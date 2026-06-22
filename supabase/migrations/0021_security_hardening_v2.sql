-- 0021_security_hardening_v2.sql
-- Second hardening pass after V102/V107 added RPCs + vendor-only tables. The
-- Supabase linter flagged two categories; this migration addresses both
-- without changing any product behavior.
--
-- 1. SECURITY DEFINER RPCs reachable via PostgREST as anon / authenticated.
--    Our RPCs already gate internally (has_permission), but defense-in-depth:
--    revoke EXECUTE from anon for the write RPCs, and from BOTH anon and
--    authenticated for has_permission (which is an internal RLS helper, never
--    meant as a user-facing API).
-- 2. Tables with RLS enabled but no policy (platform_admins, signup_intents,
--    tenant_trusted_publishers). Intent is "service-role only" — we lock the
--    intent in by revoking all client privileges. (RLS enabled + no policy
--    already denies row access, but revoking privileges also stops INSERTs
--    on tables without DEFAULT-friendly columns from spamming the WAL.)

-- 1a. has_permission: internal helper only. RLS policies call it as the
--     table owner (postgres), not as the requesting role, so removing the
--     authenticated/anon grants does not break any policy. PUBLIC was never
--     granted by 0001, but be explicit.
REVOKE EXECUTE ON FUNCTION has_permission(UUID, UUID, TEXT) FROM PUBLIC, anon, authenticated;

-- 1b. inventory_adjust + work_order_advance: stay callable by authenticated
--     (they need auth.uid() to mean something), but block anon.
REVOKE EXECUTE ON FUNCTION inventory_adjust(UUID, NUMERIC, TEXT) FROM anon;
REVOKE EXECUTE ON FUNCTION work_order_advance(UUID, TEXT, NUMERIC, TEXT, TEXT) FROM anon;

-- 2. Vendor- and service-role-only tables. RLS is on; no policy means default
--    deny. Belt-and-suspenders: revoke client privileges entirely AND add
--    explicit "no rows" policies so the linter stops flagging these. Service
--    role bypasses RLS, so it still has full access.
REVOKE ALL ON TABLE platform_admins           FROM anon, authenticated;
REVOKE ALL ON TABLE signup_intents            FROM anon, authenticated;
REVOKE ALL ON TABLE tenant_trusted_publishers FROM anon, authenticated;

DO $$ BEGIN
    EXECUTE 'DROP POLICY IF EXISTS platform_admins_deny_all ON platform_admins';
    EXECUTE 'CREATE POLICY platform_admins_deny_all ON platform_admins FOR ALL USING (false) WITH CHECK (false)';
    EXECUTE 'DROP POLICY IF EXISTS signup_intents_deny_all ON signup_intents';
    EXECUTE 'CREATE POLICY signup_intents_deny_all ON signup_intents FOR ALL USING (false) WITH CHECK (false)';
    EXECUTE 'DROP POLICY IF EXISTS tenant_trusted_publishers_deny_all ON tenant_trusted_publishers';
    EXECUTE 'CREATE POLICY tenant_trusted_publishers_deny_all ON tenant_trusted_publishers FOR ALL USING (false) WITH CHECK (false)';
END $$;

-- The remaining WARNs (inventory_adjust + work_order_advance executable by
-- `authenticated`) are intentional: those RPCs are how product users perform
-- the only writes RLS does not grant directly. The functions themselves
-- enforce tenant scope (current_tenant) and capability scope (has_permission),
-- so the SECURITY DEFINER + authenticated execute combination is the design.
-- auth_leaked_password_protection is a Supabase Auth dashboard toggle and
-- cannot be set from a migration.
