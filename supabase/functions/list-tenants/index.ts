// AETHER-OS — list-tenants edge function (vendor Console).
//
// Cross-tenant read for the vendor's control plane: every tenant + its
// subscription. RLS scopes normal clients to one tenant, so this runs with the
// service role AFTER verifying the caller is a platform_admin.
//
// Secrets (Deno.env): SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface TenantSummary {
  id: string;
  companyId: string;
  name: string;
  industrySlug: string;
  tier: string;
  status: string;
  subscriptionStatus: string;
  billingCycle: string;
}

/** Decode the caller's auth.users id (sub) from the bearer JWT (no verify —
 *  Supabase's gateway verifies before the function runs). */
export function userIdFromJwt(token: string): string | null {
  const parts = token.split('.');
  if (parts.length !== 3) return null;
  try {
    const payload = JSON.parse(atob(parts[1]!.replace(/-/g, '+').replace(/_/g, '/'))) as {
      sub?: string;
    };
    return payload.sub ?? null;
  } catch {
    return null;
  }
}

/** Merge tenant rows with their subscription rows into Console summaries. */
export function toSummaries(
  tenants: Array<Record<string, unknown>>,
  subs: Array<Record<string, unknown>>,
): TenantSummary[] {
  const byTenant = new Map<string, Record<string, unknown>>();
  for (const s of subs) byTenant.set(String(s.tenant_id), s);
  return tenants.map((t) => {
    const sub = byTenant.get(String(t.id)) ?? {};
    return {
      id: String(t.id),
      companyId: String(t.company_id ?? ''),
      name: String(t.name ?? ''),
      industrySlug: String(t.industry_slug ?? ''),
      tier: String(t.tier ?? ''),
      status: String(t.status ?? ''),
      subscriptionStatus: String(sub.status ?? 'none'),
      billingCycle: String(sub.billing_cycle ?? ''),
    };
  });
}

async function sb(url: string, key: string, path: string): Promise<Response> {
  return fetch(`${url}${path}`, {
    headers: { apikey: key, authorization: `Bearer ${key}` },
  });
}

export async function handler(req: Request): Promise<Response> {
  const json = (b: unknown, status = 200) =>
    new Response(JSON.stringify(b), { status, headers: { 'content-type': 'application/json' } });

  const url = Deno.env.get('SUPABASE_URL');
  const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY');
  if (!url || !key) return json({ error: 'not configured' }, 500);

  const bearer = (req.headers.get('authorization') ?? '').replace(/^Bearer /, '');
  const userId = userIdFromJwt(bearer);
  if (!userId) return json({ error: 'unauthenticated' }, 401);

  // Gate: caller must be a platform_admin.
  const adminRes = await sb(url, key, `/rest/v1/platform_admins?user_id=eq.${userId}&select=role`);
  const admins = (await adminRes.json()) as Array<{ role: string }>;
  if (admins.length === 0) return json({ error: 'forbidden: platform admin required' }, 403);

  const [tenantsRes, subsRes] = await Promise.all([
    sb(url, key, '/rest/v1/tenants?select=id,company_id,name,industry_slug,tier,status'),
    sb(url, key, '/rest/v1/subscriptions?select=tenant_id,status,billing_cycle'),
  ]);
  const tenants = (await tenantsRes.json()) as Array<Record<string, unknown>>;
  const subs = (await subsRes.json()) as Array<Record<string, unknown>>;
  return json({ tenants: toSummaries(tenants, subs) });
}

if (import.meta.main) {
  serve(handler);
}
