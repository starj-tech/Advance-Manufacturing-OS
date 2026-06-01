// AETHER-OS — list-modules-stats edge function (vendor Console).
//
// Cross-tenant read for the vendor's module catalog: every module + install
// count + how many tenants have it kill-switched. Gated on platform_admins.
//
// Secrets (Deno.env): SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface ModuleStat {
  id: string;
  currentVersion: string;
  status: string;
  installs: number;
  enabledInstalls: number;
}

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

export function summarize(
  modules: Array<Record<string, unknown>>,
  installs: Array<Record<string, unknown>>,
): ModuleStat[] {
  const counts = new Map<string, { installs: number; enabled: number }>();
  for (const i of installs) {
    const id = String(i.module_id);
    const cur = counts.get(id) ?? { installs: 0, enabled: 0 };
    cur.installs += 1;
    if (i.enabled === true) cur.enabled += 1;
    counts.set(id, cur);
  }
  return modules.map((m) => {
    const c = counts.get(String(m.id)) ?? { installs: 0, enabled: 0 };
    return {
      id: String(m.id ?? ''),
      currentVersion: String(m.current_version ?? ''),
      status: String(m.status ?? ''),
      installs: c.installs,
      enabledInstalls: c.enabled,
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

  const adminRes = await sb(url, key, `/rest/v1/platform_admins?user_id=eq.${userId}&select=role`);
  const admins = (await adminRes.json()) as Array<{ role: string }>;
  if (admins.length === 0) return json({ error: 'forbidden: platform admin required' }, 403);

  const [modsRes, instRes] = await Promise.all([
    sb(url, key, '/rest/v1/modules?select=id,current_version,status'),
    sb(url, key, '/rest/v1/tenant_modules?select=module_id,enabled'),
  ]);
  const modules = (await modsRes.json()) as Array<Record<string, unknown>>;
  const installs = (await instRes.json()) as Array<Record<string, unknown>>;
  return json({ modules: summarize(modules, installs) });
}

if (import.meta.main) {
  serve(handler);
}
