-- 0017_user_provisioning.sql
-- Per-user login credentials (Company ID + Username + generated password),
-- sub-roles, the new IT role, and the vendor's cross-tenant platform admins.

-- Username + sub-role + forced first-login reset on each company membership.
ALTER TABLE tenant_users ADD COLUMN IF NOT EXISTS username             TEXT;
ALTER TABLE tenant_users ADD COLUMN IF NOT EXISTS sub_role             TEXT;
ALTER TABLE tenant_users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT TRUE;

-- Username is unique within a company; login = Company ID + Username + password.
CREATE UNIQUE INDEX IF NOT EXISTS tenant_users_username_idx
    ON tenant_users (tenant_id, username) WHERE username IS NOT NULL;

-- Allow the IT role alongside the four shell roles.
ALTER TABLE tenant_users DROP CONSTRAINT IF EXISTS tenant_users_primary_role_check;
ALTER TABLE tenant_users ADD  CONSTRAINT tenant_users_primary_role_check
    CHECK (primary_role IN ('developer', 'executive', 'manager', 'employee', 'it'));

-- Vendor staff (the software owner) — a separate realm from tenant users,
-- gated by the service role in the Console Edge Functions.
CREATE TABLE IF NOT EXISTS platform_admins (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL UNIQUE REFERENCES auth.users(id) ON DELETE CASCADE,
    email      TEXT NOT NULL,
    role       TEXT NOT NULL DEFAULT 'support'
                CHECK (role IN ('vendor_admin', 'support', 'module_publisher')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Service-role only (RLS enabled, no policy = deny for anon/authenticated).
ALTER TABLE platform_admins ENABLE ROW LEVEL SECURITY;
