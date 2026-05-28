// AETHER-OS — create-checkout edge function.
//
// Records a `signup_intent` and opens a Stripe Checkout session (recurring
// subscription) for the chosen tier + billing cycle. On payment the
// stripe-webhook reads the intent and provisions the tenant + accounts.
//
// Secrets (Deno.env): STRIPE_SECRET_KEY, STRIPE_PRICE_<TIER>_<CYCLE>,
//   SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY, CHECKOUT_SUCCESS_URL,
//   CHECKOUT_CANCEL_URL. Missing billing keys => 500 (never a silent free
//   account). Pure helpers below are unit-tested without any secret.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export type TierSlug = 'standard-node' | 'advanced-automata' | 'global-enterprise';
export type BillingCycle = 'monthly' | 'annual';

export interface RosterEntry {
  name: string;
  email: string;
  role: string;
  subRole: string;
}

export interface CheckoutRequest {
  companyName: string;
  billingEmail: string;
  region: string;
  industrySlug: string;
  tier: TierSlug;
  billingCycle: BillingCycle;
  roster: RosterEntry[];
}

const TIERS: readonly TierSlug[] = ['standard-node', 'advanced-automata', 'global-enterprise'];
const CYCLES: readonly BillingCycle[] = ['monthly', 'annual'];

/** Env var name holding the Stripe price id for a tier + cycle. */
export function priceEnvVar(tier: TierSlug, cycle: BillingCycle): string {
  return `STRIPE_PRICE_${tier.toUpperCase().replace(/-/g, '_')}_${cycle.toUpperCase()}`;
}

/** Validate + normalize an incoming checkout request. Throws on invalid input. */
export function parseCheckoutRequest(body: unknown): CheckoutRequest {
  const b = (body ?? {}) as Record<string, unknown>;
  const companyName = typeof b.companyName === 'string' ? b.companyName.trim() : '';
  const billingEmail = typeof b.billingEmail === 'string' ? b.billingEmail.trim() : '';
  const tier = b.tier as TierSlug;
  const billingCycle = b.billingCycle as BillingCycle;
  const industrySlug = typeof b.industrySlug === 'string' ? b.industrySlug : '';
  const roster = Array.isArray(b.roster) ? (b.roster as RosterEntry[]) : [];

  if (!companyName) throw new Error('companyName required');
  if (!/.+@.+\..+/.test(billingEmail)) throw new Error('valid billingEmail required');
  if (!TIERS.includes(tier)) throw new Error('invalid tier');
  if (!CYCLES.includes(billingCycle)) throw new Error('invalid billingCycle');
  if (!industrySlug) throw new Error('industrySlug required');
  if (roster.length === 0) throw new Error('roster must not be empty');

  return {
    companyName,
    billingEmail,
    region: typeof b.region === 'string' ? b.region : 'us-east',
    industrySlug,
    tier,
    billingCycle,
    roster,
  };
}

/** Build the x-www-form-urlencoded body for Stripe's Checkout Session create. */
export function buildCheckoutForm(opts: {
  priceId: string;
  intentId: string;
  billingEmail: string;
  successUrl: string;
  cancelUrl: string;
}): Record<string, string> {
  return {
    mode: 'subscription',
    'line_items[0][price]': opts.priceId,
    'line_items[0][quantity]': '1',
    customer_email: opts.billingEmail,
    success_url: opts.successUrl,
    cancel_url: opts.cancelUrl,
    'metadata[signup_intent_id]': opts.intentId,
    'subscription_data[metadata][signup_intent_id]': opts.intentId,
  };
}

async function insertSignupIntent(req: CheckoutRequest): Promise<string> {
  const url = Deno.env.get('SUPABASE_URL');
  const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY');
  if (!url || !key) throw new Error('supabase not configured');
  const res = await fetch(`${url}/rest/v1/signup_intents`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      apikey: key,
      authorization: `Bearer ${key}`,
      prefer: 'return=representation',
    },
    body: JSON.stringify({
      billing_email: req.billingEmail,
      company_name: req.companyName,
      region: req.region,
      industry_slug: req.industrySlug,
      tier: req.tier,
      billing_cycle: req.billingCycle,
      roster: req.roster,
      status: 'pending',
    }),
  });
  if (!res.ok) throw new Error(`signup_intent insert failed: ${res.status}`);
  const rows = (await res.json()) as Array<{ id: string }>;
  return rows[0]!.id;
}

export async function handler(req: Request): Promise<Response> {
  const json = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

  if (req.method !== 'POST') return json({ error: 'method not allowed' }, 405);

  let parsed: CheckoutRequest;
  try {
    parsed = parseCheckoutRequest(await req.json());
  } catch (e) {
    return json({ error: (e as Error).message }, 400);
  }

  const stripeKey = Deno.env.get('STRIPE_SECRET_KEY');
  const priceId = Deno.env.get(priceEnvVar(parsed.tier, parsed.billingCycle));
  if (!stripeKey || !priceId) return json({ error: 'billing not configured' }, 500);

  const intentId = await insertSignupIntent(parsed);
  const form = buildCheckoutForm({
    priceId,
    intentId,
    billingEmail: parsed.billingEmail,
    successUrl: (Deno.env.get('CHECKOUT_SUCCESS_URL') ?? '').replace('{INTENT}', intentId),
    cancelUrl: Deno.env.get('CHECKOUT_CANCEL_URL') ?? '',
  });
  const res = await fetch('https://api.stripe.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      authorization: `Bearer ${stripeKey}`,
      'content-type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams(form).toString(),
  });
  if (!res.ok) return json({ error: 'stripe error' }, 502);
  const session = (await res.json()) as { id: string; url: string };
  return json({ checkoutUrl: session.url, intentId });
}

if (import.meta.main) {
  serve(handler);
}
