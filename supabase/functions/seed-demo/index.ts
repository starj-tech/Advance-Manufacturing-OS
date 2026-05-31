// AETHER-OS — seed-demo edge function (dev utility).
//
// One-shot, IDEMPOTENT seeding for a fresh project so the live Console / IT /
// Main apps are demonstrable without going through Stripe onboarding. Creates:
//   - a vendor platform_admin (email login)
//   - a demo company tenant (Company-ID login) with roles + permissions
//   - 3 member accounts (executive / manager / it) ready to sign in
//   - a few machines + work orders
//
// Self-disabling: if a platform_admin already exists it refuses (returns
// alreadySeeded). Returns generated credentials ONCE. Uses the auto-injected
// SUPABASE_URL / SUPABASE_SERVICE_ROLE_KEY. Safe to delete after seeding.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

const PW_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789';

function pw(n = 14): string {
  const b = new Uint8Array(n);
  crypto.getRandomValues(b);
  let out = '';
  for (const x of b) out += PW_ALPHABET[x % PW_ALPHABET.length];
  return out;
}

const COMPANY_ID = 'DEMO-0001';
const SYNTH_DOMAIN = COMPANY_ID.toLowerCase() + '.tenant.aether-os.internal';

const ROLE_PERMS: Record<string, string[]> = {
  developer: ['*'],
  executive: ['kpi:read', 'twin:read', 'finance:read'],
  manager: [
    'work_orders:read',
    'work_orders:create',
    'work_orders:update',
    'work_orders:approve',
    'machines:read',
    'inventory:read',
    'inventory:adjust',
    'maintenance:read',
    'tenant:configure',
  ],
  employee: ['tasks:read', 'tasks:complete', 'sos:trigger', 'clock:write'],
  it: ['users:*', 'devices:*', 'modules:*', 'audit:read', 'security:*'],
};

interface Ctx {
  url: string;
  key: string;
}

async function rest(
  ctx: Ctx,
  path: string,
  method: string,
  body?: unknown,
  representation = false,
): Promise<Response> {
  const headers: Record<string, string> = {
    'content-type': 'application/json',
    apikey: ctx.key,
    authorization: 'Bearer ' + ctx.key,
  };
  if (representation) headers.prefer = 'return=representation';
  return fetch(ctx.url + path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

async function createUser(
  ctx: Ctx,
  email: string,
  password: string,
  appMeta: Record<string, unknown>,
  fullName: string,
): Promise<string | null> {
  const res = await rest(ctx, '/auth/v1/admin/users', 'POST', {
    email,
    password,
    email_confirm: true,
    app_metadata: appMeta,
    user_metadata: { full_name: fullName },
  });
  const u = (await res.json()) as { id?: string };
  return u.id ?? null;
}

export async function handler(req: Request): Promise<Response> {
  const json = (b: unknown, status = 200) =>
    new Response(JSON.stringify(b), { status, headers: { 'content-type': 'application/json' } });
  if (req.method !== 'POST') return json({ error: 'method not allowed' }, 405);

  const url = Deno.env.get('SUPABASE_URL');
  const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY');
  if (!url || !key) return json({ error: 'not configured' }, 500);
  const ctx: Ctx = { url, key };

  // Idempotency guard.
  const existing = await rest(ctx, '/rest/v1/platform_admins?select=id&limit=1', 'GET');
  const rows = (await existing.json()) as unknown[];
  if (Array.isArray(rows) && rows.length > 0) return json({ alreadySeeded: true });

  // 1) Vendor platform admin (email login for the Console).
  const vendorEmail = 'admin@aether-os.dev';
  const vendorPw = pw(16);
  const vendorId = await createUser(
    ctx,
    vendorEmail,
    vendorPw,
    { role: 'vendor_admin' },
    'Vendor Admin',
  );
  if (!vendorId) return json({ error: 'vendor user create failed' }, 502);
  await rest(ctx, '/rest/v1/platform_admins', 'POST', {
    user_id: vendorId,
    email: vendorEmail,
    role: 'vendor_admin',
  });

  // 2) Demo company tenant.
  const tRes = await rest(
    ctx,
    '/rest/v1/tenants',
    'POST',
    {
      slug: COMPANY_ID.toLowerCase(),
      name: 'Demo Manufaktur',
      region: 'ap-southeast-1',
      company_id: COMPANY_ID,
      industry_slug: 'food-and-beverage',
      tier: 'advanced-automata',
      billing_email: 'demo@aether-os.dev',
      status: 'active',
    },
    true,
  );
  const tenant = ((await tRes.json()) as Array<{ id: string }>)[0];
  if (!tenant) return json({ error: 'tenant create failed' }, 502);
  const tenantId = tenant.id;

  await rest(ctx, '/rest/v1/tenant_industry', 'POST', {
    tenant_id: tenantId,
    industry_slug: 'food-and-beverage',
  });

  // 3) Roles + permissions.
  for (const role of Object.keys(ROLE_PERMS)) {
    await rest(ctx, '/rest/v1/roles', 'POST', {
      tenant_id: tenantId,
      role_name: role,
      description: role,
    });
    for (const scope of ROLE_PERMS[role]!) {
      await rest(ctx, '/rest/v1/permissions', 'POST', {
        tenant_id: tenantId,
        role_name: role,
        scope,
        granted: true,
      });
    }
  }

  // 4) Member accounts (Company-ID login, ready to use immediately).
  const members = [
    { username: 'ceo', role: 'executive', subRole: 'CEO', name: 'Dewi (CEO)' },
    { username: 'prod', role: 'manager', subRole: 'Production', name: 'Andi (Prod Manager)' },
    { username: 'itadmin', role: 'it', subRole: 'IT Admin', name: 'Sari (IT)' },
  ];
  const created: Array<{ username: string; role: string; password: string }> = [];
  for (const m of members) {
    const password = pw(12);
    const uid = await createUser(
      ctx,
      m.username + '@' + SYNTH_DOMAIN,
      password,
      {
        tenant_id: tenantId,
        company_id: COMPANY_ID,
        primary_role: m.role,
        sub_role: m.subRole,
        permissions: ROLE_PERMS[m.role],
        must_change_password: false,
      },
      m.name,
    );
    if (!uid) continue;
    await rest(ctx, '/rest/v1/users', 'POST', { id: uid, status: 'active' });
    await rest(ctx, '/rest/v1/tenant_users', 'POST', {
      tenant_id: tenantId,
      user_id: uid,
      primary_role: m.role,
      sub_role: m.subRole,
      username: m.username,
      must_change_password: false,
    });
    created.push({ username: m.username, role: m.role, password });
  }

  // 5) Machines + work orders so the Main product shows live data.
  const machines = [
    { code: 'PRESS-01', name: 'Hydraulic press 250t', status: 'running' },
    { code: 'CNC-12', name: 'CNC mill A12', status: 'idle' },
    { code: 'WELD-04', name: 'Robotic welder W4', status: 'fault' },
  ];
  for (const mc of machines) {
    await rest(ctx, '/rest/v1/machines', 'POST', {
      tenant_id: tenantId,
      code: mc.code,
      name: mc.name,
      status: mc.status,
      hlc: '0.0.seed',
    });
  }
  const workOrders = [
    { code: 'WO-2026-0042', status: 'running', qty_planned: 150, qty_done: 120 },
    { code: 'WO-2026-0043', status: 'released', qty_planned: 80, qty_done: 0 },
  ];
  for (const wo of workOrders) {
    await rest(ctx, '/rest/v1/work_orders', 'POST', {
      tenant_id: tenantId,
      code: wo.code,
      status: wo.status,
      qty_planned: wo.qty_planned,
      qty_done: wo.qty_done,
      hlc: '0.0.seed',
    });
  }

  // 6) Materials so the manager's Inventory page has real rows on first login.
  const materials = [
    { sku: 'STR-CORE-3K', uom: 'pcs', qty_on_hand: 320, qty_reserved: 80 },
    { sku: 'RTR-SHAFT-12', uom: 'pcs', qty_on_hand: 540, qty_reserved: 200 },
  ];
  for (const mat of materials) {
    await rest(ctx, '/rest/v1/materials', 'POST', {
      tenant_id: tenantId,
      sku: mat.sku,
      uom: mat.uom,
      qty_on_hand: mat.qty_on_hand,
      qty_reserved: mat.qty_reserved,
      hlc: '0.0.seed',
    });
  }

  return json({
    seeded: true,
    vendorConsole: { email: vendorEmail, password: vendorPw },
    company: { companyId: COMPANY_ID, members: created },
  });
}

if (import.meta.main) {
  serve(handler);
}
