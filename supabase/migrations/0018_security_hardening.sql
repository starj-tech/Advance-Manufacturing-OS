-- 0018_security_hardening.sql
-- Close RLS gaps flagged by the Supabase database linter: enable RLS on the
-- `users` and `modules` tables (they were exposed to PostgREST without RLS),
-- add the tenant-scoped read policies the apps rely on, and pin a stable
-- search_path on the SECURITY DEFINER / STABLE helper functions.

-- A user may read only their own row.
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
CREATE POLICY users_self_select ON users FOR SELECT USING (id = auth.uid());

-- Module registry is global; authenticated users may read published modules.
ALTER TABLE modules ENABLE ROW LEVEL SECURITY;
CREATE POLICY modules_published_select ON modules
    FOR SELECT TO authenticated USING (status = 'published');

-- Tenant-scoped reads the four apps depend on (IT roster, session bootstrap,
-- permission/role lookups, installed modules).
CREATE POLICY tenant_isolation_select_tenant_users ON tenant_users
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_tenants ON tenants
    FOR SELECT USING (id = current_tenant());
CREATE POLICY tenant_isolation_select_roles ON roles
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_permissions ON permissions
    FOR SELECT USING (tenant_id = current_tenant());
CREATE POLICY tenant_isolation_select_tenant_modules ON tenant_modules
    FOR SELECT USING (tenant_id = current_tenant());

-- Pin search_path on the helpers (linter 0011).
ALTER FUNCTION current_tenant() SET search_path = public, auth;
ALTER FUNCTION has_permission(UUID, UUID, TEXT) SET search_path = public, auth;
