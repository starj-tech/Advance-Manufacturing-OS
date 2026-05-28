-- 0016_saas_billing.sql
-- Commercial SaaS layer: company identity, subscription tiers, and the
-- pre-payment signup intent that the Stripe webhook provisions a tenant from.

-- Company-level commercial fields on the existing tenant.
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS company_id    TEXT UNIQUE; -- human-readable login id (e.g. ACME-7Q2F)
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS industry_slug TEXT;
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS tier          TEXT
    CHECK (tier IN ('standard-node', 'advanced-automata', 'global-enterprise'));
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS status        TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'suspended', 'canceled'));
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS billing_email TEXT; -- operational contact for invoices/welcome email

-- Stripe subscription mirror. Stripe is the source of truth; this is the
-- local read model kept in sync by the stripe-webhook Edge Function.
CREATE TABLE IF NOT EXISTS subscriptions (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id              UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    stripe_customer_id     TEXT NOT NULL,
    stripe_subscription_id TEXT NOT NULL UNIQUE,
    tier                   TEXT NOT NULL
                            CHECK (tier IN ('standard-node', 'advanced-automata', 'global-enterprise')),
    billing_cycle          TEXT NOT NULL CHECK (billing_cycle IN ('monthly', 'annual')),
    status                 TEXT NOT NULL DEFAULT 'active'
                            CHECK (status IN ('trialing', 'active', 'past_due', 'canceled', 'incomplete')),
    current_period_end     TIMESTAMPTZ,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS subscriptions_tenant_idx ON subscriptions (tenant_id);

-- Pre-payment onboarding capture. The Main app writes this before Stripe
-- Checkout; the stripe-webhook reads it on `checkout.session.completed` to
-- provision the tenant + all accounts, then flips status to 'provisioned'.
CREATE TABLE IF NOT EXISTS signup_intents (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    billing_email           TEXT NOT NULL,
    company_name            TEXT NOT NULL,
    region                  TEXT NOT NULL DEFAULT 'us-east',
    industry_slug           TEXT NOT NULL,
    tier                    TEXT NOT NULL
                             CHECK (tier IN ('standard-node', 'advanced-automata', 'global-enterprise')),
    billing_cycle           TEXT NOT NULL CHECK (billing_cycle IN ('monthly', 'annual')),
    roster                  JSONB NOT NULL DEFAULT '[]'::jsonb, -- [{ name, email, role, sub_role }]
    stripe_checkout_session TEXT,
    status                  TEXT NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending', 'paid', 'provisioned', 'failed')),
    provisioned_tenant_id   UUID REFERENCES tenants(id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- RLS: a tenant's members may read their own subscription; signup_intents are
-- service-role only (RLS enabled with no policy = deny for anon/authenticated).
ALTER TABLE subscriptions  ENABLE ROW LEVEL SECURITY;
ALTER TABLE signup_intents ENABLE ROW LEVEL SECURITY;

CREATE POLICY subscriptions_tenant_select ON subscriptions
    FOR SELECT USING (tenant_id = current_tenant());
