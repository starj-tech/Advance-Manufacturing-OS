// AETHER-OS — stripe-webhook edge function.
//
// On `checkout.session.completed` it provisions the tenant from the
// signup_intent: generates a Company ID, creates one auth account per roster
// row (synthetic email `username@companyid.tenant.aether-os.internal` +
// generated password, must_change_password), records the subscription, and
// triggers send-welcome-email. Stripe signature is verified first.
//
// Secrets (Deno.env): STRIPE_WEBHOOK_SECRET, SUPABASE_URL,
//   SUPABASE_SERVICE_ROLE_KEY, WELCOME_EMAIL_URL. The pure helpers (signature
//   verify, account planning) are unit-tested without any secret.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export interface RosterEntry {
  name: string;
  email: string;
  role: string;
  subRole: string;
}

export interface Account extends RosterEntry {
  username: string;
  password: string;
}

const PW_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789';

/** Cryptographically-random password from an unambiguous alphabet. */
export function generatePassword(len = 16): string {
  const bytes = new Uint8Array(len);
  crypto.getRandomValues(bytes);
  let out = '';
  for (const b of bytes) out += PW_ALPHABET[b % PW_ALPHABET.length];
  return out;
}

/** Random uppercase-alnum suffix for the Company ID. */
export function randomSuffix(len = 4): string {
  const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
  const bytes = new Uint8Array(len);
  crypto.getRandomValues(bytes);
  let out = '';
  for (const b of bytes) out += alphabet[b % alphabet.length];
  return out;
}

export function slugifyCompany(name: string): string {
  return (
    name
      .toUpperCase()
      .replace(/[^A-Z0-9]+/g, '')
      .slice(0, 8) || 'TENANT'
  );
}

/** Human-readable, unique-ish company login id, e.g. `ACME-7Q2F`. */
export function companyId(name: string, suffix: string): string {
  return `${slugifyCompany(name)}-${suffix}`;
}

/** Synthetic email mapping Company-ID + Username onto Supabase Auth. */
export function syntheticEmail(username: string, cid: string): string {
  return `${username}@${cid.toLowerCase()}.tenant.aether-os.internal`;
}

function usernameBase(entry: RosterEntry): string {
  const local = entry.email.includes('@') ? entry.email.split('@')[0]! : '';
  const base = (local || entry.name || 'user')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '.')
    .replace(/^\.+|\.+$/g, '');
  return base || 'user';
}

/** Assign a unique username + a password to each roster entry. The password
 *  generator is injectable so the dedupe logic is deterministically testable. */
export function planAccounts(
  roster: RosterEntry[],
  password: () => string = () => generatePassword(),
): Account[] {
  const taken = new Set<string>();
  return roster.map((entry) => {
    const base = usernameBase(entry);
    let username = base;
    let n = 1;
    while (taken.has(username)) {
      n += 1;
      username = `${base}${n}`;
    }
    taken.add(username);
    return { ...entry, username, password: password() };
  });
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

/** Verify a Stripe `Stripe-Signature` header (t=,v1=) against the raw body. */
export async function verifyStripeSignature(
  payload: string,
  header: string,
  secret: string,
  nowSec: number = Math.floor(Date.now() / 1000),
  toleranceSec = 300,
): Promise<boolean> {
  const parts: Record<string, string> = {};
  for (const kv of header.split(',')) {
    const idx = kv.indexOf('=');
    if (idx > 0) parts[kv.slice(0, idx).trim()] = kv.slice(idx + 1).trim();
  }
  const t = parts['t'];
  const v1 = parts['v1'];
  if (!t || !v1) return false;
  if (Math.abs(nowSec - Number(t)) > toleranceSec) return false;
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const mac = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(`${t}.${payload}`));
  return timingSafeEqual(toHex(new Uint8Array(mac)), v1);
}

interface SupabaseCtx {
  url: string;
  key: string;
}

async function sb(ctx: SupabaseCtx, path: string, init: RequestInit): Promise<Response> {
  return fetch(`${ctx.url}${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      apikey: ctx.key,
      authorization: `Bearer ${ctx.key}`,
      ...(init.headers ?? {}),
    },
  });
}

export async function handler(req: Request): Promise<Response> {
  if (req.method !== 'POST') return new Response('method not allowed', { status: 405 });

  const secret = Deno.env.get('STRIPE_WEBHOOK_SECRET');
  const supaUrl = Deno.env.get('SUPABASE_URL');
  const supaKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY');
  if (!secret || !supaUrl || !supaKey) {
    return new Response(JSON.stringify({ error: 'webhook not configured' }), { status: 500 });
  }

  const payload = await req.text();
  const sig = req.headers.get('stripe-signature') ?? '';
  if (!(await verifyStripeSignature(payload, sig, secret))) {
    return new Response(JSON.stringify({ error: 'bad signature' }), { status: 400 });
  }

  const event = JSON.parse(payload) as {
    type: string;
    data: { object: Record<string, unknown> };
  };
  if (event.type !== 'checkout.session.completed') {
    return new Response(JSON.stringify({ ignored: event.type }), { status: 200 });
  }

  const session = event.data.object;
  const meta = (session.metadata ?? {}) as Record<string, string>;
  const intentId = meta.signup_intent_id;
  if (!intentId) return new Response(JSON.stringify({ error: 'no intent' }), { status: 400 });

  const ctx: SupabaseCtx = { url: supaUrl, key: supaKey };

  // Load the intent (idempotent: skip if already provisioned).
  const intentRes = await sb(ctx, `/rest/v1/signup_intents?id=eq.${intentId}&select=*`, {
    method: 'GET',
  });
  const intents = (await intentRes.json()) as Array<{
    status: string;
    company_name: string;
    billing_email: string;
    region: string;
    industry_slug: string;
    tier: string;
    billing_cycle: string;
    roster: RosterEntry[];
  }>;
  const intent = intents[0];
  if (!intent) return new Response(JSON.stringify({ error: 'intent not found' }), { status: 404 });
  if (intent.status === 'provisioned')
    return new Response(JSON.stringify({ ok: true }), { status: 200 });

  const cid = companyId(intent.company_name, randomSuffix());
  const accounts = planAccounts(intent.roster);

  // Create the tenant.
  const tenantRes = await sb(ctx, '/rest/v1/tenants', {
    method: 'POST',
    headers: { prefer: 'return=representation' },
    body: JSON.stringify({
      slug: cid.toLowerCase(),
      name: intent.company_name,
      region: intent.region,
      company_id: cid,
      industry_slug: intent.industry_slug,
      tier: intent.tier,
      billing_email: intent.billing_email,
      status: 'active',
    }),
  });
  const tenant = ((await tenantRes.json()) as Array<{ id: string }>)[0];
  if (!tenant)
    return new Response(JSON.stringify({ error: 'tenant create failed' }), { status: 502 });

  // Record the subscription mirror.
  await sb(ctx, '/rest/v1/subscriptions', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: tenant.id,
      stripe_customer_id: String(session.customer ?? ''),
      stripe_subscription_id: String(session.subscription ?? `cs_${intentId}`),
      tier: intent.tier,
      billing_cycle: intent.billing_cycle,
      status: 'active',
    }),
  });

  // Create one auth user + membership per roster row.
  for (const acct of accounts) {
    const permissions = [`${acct.role}:*`];
    const userRes = await sb(ctx, '/auth/v1/admin/users', {
      method: 'POST',
      body: JSON.stringify({
        email: syntheticEmail(acct.username, cid),
        password: acct.password,
        email_confirm: true,
        app_metadata: {
          tenant_id: tenant.id,
          primary_role: acct.role,
          sub_role: acct.subRole,
          permissions,
        },
        user_metadata: { full_name: acct.name },
      }),
    });
    const user = (await userRes.json()) as { id?: string };
    if (!user.id) continue;
    await sb(ctx, '/rest/v1/users', {
      method: 'POST',
      headers: { prefer: 'resolution=ignore-duplicates' },
      body: JSON.stringify({ id: user.id, status: 'active' }),
    });
    await sb(ctx, '/rest/v1/tenant_users', {
      method: 'POST',
      body: JSON.stringify({
        tenant_id: tenant.id,
        user_id: user.id,
        primary_role: acct.role,
        sub_role: acct.subRole,
        username: acct.username,
        must_change_password: true,
      }),
    });
  }

  await sb(ctx, `/rest/v1/signup_intents?id=eq.${intentId}`, {
    method: 'PATCH',
    body: JSON.stringify({ status: 'provisioned', provisioned_tenant_id: tenant.id }),
  });

  // Fire-and-forget the welcome email.
  const welcomeUrl = Deno.env.get('WELCOME_EMAIL_URL');
  if (welcomeUrl) {
    await fetch(welcomeUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${supaKey}` },
      body: JSON.stringify({
        companyName: intent.company_name,
        companyId: cid,
        billingEmail: intent.billing_email,
        tier: intent.tier,
        accounts: accounts.map((a) => ({ name: a.name, username: a.username, role: a.role })),
      }),
    }).catch(() => undefined);
  }

  return new Response(JSON.stringify({ ok: true, companyId: cid, accounts: accounts.length }), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

if (import.meta.main) {
  serve(handler);
}
