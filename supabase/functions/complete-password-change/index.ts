// AETHER-OS — complete-password-change edge function.
//
// Called by the forced-reset screen after the IT admin (or initial provisioner)
// flipped must_change_password=true. The user's own JWT can update their
// password but NOT their app_metadata, so this service-role function does both
// atomically: validate strength, set the new password, clear the flag, audit.
//
// Secrets (Deno.env): SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface Caller {
  userId: string;
  tenantId: string | null;
}

/** Decode the caller from JWT (gateway verifies signature). */
export function callerFromJwt(token: string): Caller | null {
  const parts = token.split('.');
  if (parts.length !== 3) return null;
  try {
    const payload = JSON.parse(atob(parts[1]!.replace(/-/g, '+').replace(/_/g, '/'))) as {
      sub?: string;
      app_metadata?: { tenant_id?: string };
    };
    if (!payload.sub) return null;
    return { userId: payload.sub, tenantId: payload.app_metadata?.tenant_id ?? null };
  } catch {
    return null;
  }
}

/** Minimum-strength check the function enforces server-side. */
export function strongEnough(pw: string): boolean {
  if (pw.length < 10) return false;
  const hasLetter = /[A-Za-z]/.test(pw);
  const hasDigit = /\d/.test(pw);
  return hasLetter && hasDigit;
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
  const caller = callerFromJwt(bearer);
  if (!caller) return json({ error: 'unauthenticated' }, 401);

  const body = (await req.json().catch(() => ({}))) as { newPassword?: unknown };
  const newPassword = typeof body.newPassword === 'string' ? body.newPassword : '';
  if (!strongEnough(newPassword)) {
    return json({ error: 'password must be 10+ chars and mix letters + digits' }, 400);
  }

  // Read current app_metadata so we PRESERVE everything else when clearing
  // must_change_password. (PUT replaces, not merges, for app_metadata on some
  // versions, so we explicitly merge.)
  const userRes = await sb(url, key, `/auth/v1/admin/users/${caller.userId}`, { method: 'GET' });
  if (!userRes.ok) return json({ error: 'lookup failed' }, 502);
  const userJson = (await userRes.json()) as { app_metadata?: Record<string, unknown> };
  const nextMeta = { ...(userJson.app_metadata ?? {}), must_change_password: false };

  const updateRes = await sb(url, key, `/auth/v1/admin/users/${caller.userId}`, {
    method: 'PUT',
    body: JSON.stringify({ password: newPassword, app_metadata: nextMeta }),
  });
  if (!updateRes.ok) return json({ error: 'update failed' }, 502);

  // Audit (best-effort; ignore failures) — only when the user has a tenant.
  if (caller.tenantId) {
    await sb(url, key, '/rest/v1/audit_log', {
      method: 'POST',
      body: JSON.stringify({
        tenant_id: caller.tenantId,
        actor_id: caller.userId,
        action: 'password_changed',
        resource: 'users',
        resource_id: caller.userId,
        metadata: { self: true },
        hlc: (Date.now() / 1000).toString() + '.0.fn',
      }),
    });
  }

  return json({ ok: true });
}

if (import.meta.main) {
  serve(handler);
}
