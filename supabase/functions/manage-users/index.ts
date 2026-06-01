// AETHER-OS — manage-users edge function (IT app).
//
// Service-role operations the client IT team performs on their own tenant:
//   - reset-password: set a new generated password + must_change_password
//   - import-roster:  bulk-create accounts from a CSV (same shape as onboarding)
// The caller's JWT is checked for the `it` role + matching tenant before any
// admin action. Pure helpers (CSV parse, password gen, username dedupe) are
// unit-tested without secrets.
//
// Secrets (Deno.env): SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface RosterEntry {
  name: string;
  email: string;
  role: string;
  subRole: string;
}

const VALID_ROLES = ['developer', 'executive', 'manager', 'employee', 'it'];
const PW_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789';

/** Cryptographically-random password from an unambiguous alphabet. */
export function generatePassword(len = 16): string {
  const bytes = new Uint8Array(len);
  crypto.getRandomValues(bytes);
  let out = '';
  for (const b of bytes) out += PW_ALPHABET[b % PW_ALPHABET.length];
  return out;
}

/** Parse a roster CSV (name,email,role,sub_role); header- and blank-tolerant. */
export function parseRosterCsv(text: string): RosterEntry[] {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length === 0) return [];
  const first = lines[0]!.toLowerCase();
  const rows = first.startsWith('name') && first.includes('email') ? lines.slice(1) : lines;
  const out: RosterEntry[] = [];
  for (const line of rows) {
    const c = line.split(',').map((x) => x.trim());
    const name = c[0] ?? '';
    const email = c[1] ?? '';
    if (!name && !email) continue;
    const role = c[2] ?? 'employee';
    out.push({
      name,
      email,
      role: VALID_ROLES.includes(role) ? role : 'employee',
      subRole: c[3] ?? '',
    });
  }
  return out;
}

/** Username from an entry's email local-part or name (lowercased, dotted). */
export function usernameFor(entry: RosterEntry): string {
  const local = entry.email.includes('@') ? entry.email.split('@')[0]! : '';
  const base = (local || entry.name || 'user')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '.')
    .replace(/^\.+|\.+$/g, '');
  return base || 'user';
}

interface Caller {
  role: string;
  tenantId: string;
  userId: string | null;
}

/** Extract role + tenant + user id from a Supabase JWT's app_metadata + sub
 *  claim. The function gateway already verified the JWT signature before this
 *  handler runs, so we just decode it. */
export function callerFromJwt(token: string): Caller | null {
  const parts = token.split('.');
  if (parts.length !== 3) return null;
  try {
    const payload = JSON.parse(atob(parts[1]!.replace(/-/g, '+').replace(/_/g, '/'))) as {
      sub?: string;
      app_metadata?: { primary_role?: string; tenant_id?: string };
    };
    const meta = payload.app_metadata ?? {};
    if (!meta.primary_role || !meta.tenant_id) return null;
    return { role: meta.primary_role, tenantId: meta.tenant_id, userId: payload.sub ?? null };
  } catch {
    return null;
  }
}

export function authorizeIt(caller: Caller | null): boolean {
  return caller?.role === 'it';
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
  if (!authorizeIt(caller)) return json({ error: 'forbidden: it role required' }, 403);

  const body = (await req.json().catch(() => ({}))) as Record<string, unknown>;
  const action = body.action;

  if (action === 'reset-password') {
    const userId = String(body.userId ?? '');
    if (!userId) return json({ error: 'userId required' }, 400);

    // Ensure target user belongs to the caller's tenant — IT can't reach across
    // tenants. (RLS would also block if we used the caller's JWT, but we're on
    // the service role here, so do it explicitly.)
    const own = await sb(
      url,
      key,
      `/rest/v1/tenant_users?tenant_id=eq.${caller!.tenantId}&user_id=eq.${userId}&select=user_id&limit=1`,
      { method: 'GET' },
    );
    const ownRows = (await own.json()) as unknown[];
    if (!Array.isArray(ownRows) || ownRows.length === 0) {
      return json({ error: 'target user not in caller tenant' }, 403);
    }

    const password = generatePassword();
    const res = await sb(url, key, `/auth/v1/admin/users/${userId}`, {
      method: 'PUT',
      body: JSON.stringify({ password, app_metadata: { must_change_password: true } }),
    });
    if (!res.ok) return json({ error: 'reset failed' }, 502);

    // Audit so the developer's Audit Log shell sees the reset event.
    await sb(url, key, '/rest/v1/audit_log', {
      method: 'POST',
      body: JSON.stringify({
        tenant_id: caller!.tenantId,
        actor_id: caller!.userId ?? null,
        action: 'password_reset',
        resource: 'users',
        resource_id: userId,
        metadata: { by_role: caller!.role },
        hlc: (Date.now() / 1000).toString() + '.0.fn',
      }),
    });

    // The new password is returned once to the IT admin to hand off securely.
    return json({ ok: true, password });
  }

  if (action === 'import-roster') {
    const entries = parseRosterCsv(String(body.csv ?? ''));
    const companyId = String(body.companyId ?? '');
    if (!companyId) return json({ error: 'companyId required' }, 400);

    const tenantId = caller!.tenantId;
    const synthDomain = `${companyId.toLowerCase()}.tenant.aether-os.internal`;
    const created: Array<{ username: string; role: string; password: string }> = [];
    const skipped: Array<{ username: string; reason: string }> = [];

    for (const entry of entries) {
      const username = usernameFor(entry);
      // Skip if user already exists for this tenant.
      const exists = await sb(
        url,
        key,
        `/rest/v1/tenant_users?tenant_id=eq.${tenantId}&username=eq.${username}&select=user_id&limit=1`,
        { method: 'GET' },
      );
      const existsRows = (await exists.json()) as unknown[];
      if (Array.isArray(existsRows) && existsRows.length > 0) {
        skipped.push({ username, reason: 'already exists' });
        continue;
      }

      const password = generatePassword();
      const synthEmail = `${username}@${synthDomain}`;
      const userRes = await sb(url, key, '/auth/v1/admin/users', {
        method: 'POST',
        body: JSON.stringify({
          email: synthEmail,
          password,
          email_confirm: true,
          app_metadata: {
            tenant_id: tenantId,
            company_id: companyId,
            primary_role: entry.role,
            sub_role: entry.subRole,
            must_change_password: true,
          },
          user_metadata: { full_name: entry.name },
        }),
      });
      if (!userRes.ok) {
        skipped.push({ username, reason: `auth ${userRes.status}` });
        continue;
      }
      const user = (await userRes.json()) as { id?: string };
      if (!user.id) {
        skipped.push({ username, reason: 'no id returned' });
        continue;
      }

      await sb(url, key, '/rest/v1/users', {
        method: 'POST',
        body: JSON.stringify({ id: user.id, status: 'active' }),
      });
      await sb(url, key, '/rest/v1/tenant_users', {
        method: 'POST',
        body: JSON.stringify({
          tenant_id: tenantId,
          user_id: user.id,
          primary_role: entry.role,
          sub_role: entry.subRole,
          username,
          must_change_password: true,
        }),
      });
      await sb(url, key, '/rest/v1/audit_log', {
        method: 'POST',
        body: JSON.stringify({
          tenant_id: tenantId,
          actor_id: caller!.userId,
          action: 'user_provisioned',
          resource: 'users',
          resource_id: user.id,
          metadata: { username, role: entry.role, sub_role: entry.subRole, via: 'csv_import' },
          hlc: (Date.now() / 1000).toString() + '.0.fn',
        }),
      });
      created.push({ username, role: entry.role, password });
    }

    return json({ ok: true, created, skipped });
  }

  return json({ error: 'unknown action' }, 400);
}

if (import.meta.main) {
  serve(handler);
}
