// AETHER-OS — set-tenant-status edge function (vendor Console).
//
// Vendor-only action to suspend or reactivate a client tenant. Gated on the
// platform_admins table, audited to the target tenant's audit_log. Used by the
// Console's TenantsPage to take a misbehaving / unpaid tenant offline without
// touching their auth users (suspension is a status flag, not account
// deletion).
//
// Secrets (Deno.env): SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export type TenantStatus = 'active' | 'suspended';

export function validStatus(s: unknown): s is TenantStatus {
  return s === 'active' || s === 'suspended';
}

/** Decode the caller's auth.users id (sub) from the bearer JWT — the function
 *  gateway already verified the signature. */
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

async function sb(url: string, key: string, path: string, init: RequestInit): Promise<Response> {
  return fetch(`${url}${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      apikey: key,
      authorization: `Bearer ${key}`,
      ...(init.headers ?? {}),
    },
  });
}

export async function handler(req: Request): Promise<Response> {
  const json = (b: unknown, status = 200) =>
    new Response(JSON.stringify(b), { status, headers: { 'content-type': 'application/json' } });

  if (req.method !== 'POST') return json({ error: 'method not allowed' }, 405);

  const url = Deno.env.get('SUPABASE_URL');
  const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY');
  if (!url || !key) return json({ error: 'not configured' }, 500);

  const bearer = (req.headers.get('authorization') ?? '').replace(/^Bearer /, '');
  const userId = userIdFromJwt(bearer);
  if (!userId) return json({ error: 'unauthenticated' }, 401);

  // Gate on platform_admins.
  const admRes = await sb(url, key, `/rest/v1/platform_admins?user_id=eq.${userId}&select=role`, {
    method: 'GET',
  });
  const admins = (await admRes.json()) as Array<{ role: string }>;
  if (!Array.isArray(admins) || admins.length === 0) {
    return json({ error: 'forbidden: platform admin required' }, 403);
  }

  const body = (await req.json().catch(() => ({}))) as Record<string, unknown>;
  const tenantId = String(body.tenantId ?? '');
  const next = body.status;
  if (!tenantId) return json({ error: 'tenantId required' }, 400);
  if (!validStatus(next)) return json({ error: 'status must be active or suspended' }, 400);
  const reason = typeof body.reason === 'string' ? body.reason : null;

  const patch = await sb(url, key, `/rest/v1/tenants?id=eq.${tenantId}`, {
    method: 'PATCH',
    headers: { prefer: 'return=representation' },
    body: JSON.stringify({ status: next }),
  });
  if (!patch.ok) return json({ error: 'patch failed' }, 502);
  const rows = (await patch.json()) as Array<{ id: string }>;
  if (rows.length === 0) return json({ error: 'tenant not found' }, 404);

  // Audit on the target tenant's own log so the developer shell there sees it.
  await sb(url, key, '/rest/v1/audit_log', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: tenantId,
      actor_id: userId,
      action: next === 'suspended' ? 'tenant_suspended' : 'tenant_reactivated',
      resource: 'tenants',
      resource_id: tenantId,
      metadata: { by_role: admins[0]!.role, reason },
      hlc: (Date.now() / 1000).toString() + '.0.fn',
    }),
  });

  return json({ ok: true, tenantId, status: next });
}

if (import.meta.main) {
  serve(handler);
}
