// Tests for create-checkout pure helpers. Run with `deno test --allow-env`.
import { assertEquals, assertThrows } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { buildCheckoutForm, parseCheckoutRequest, priceEnvVar } from './index.ts';

Deno.test('priceEnvVar maps tier+cycle to the env var name', () => {
  assertEquals(priceEnvVar('standard-node', 'monthly'), 'STRIPE_PRICE_STANDARD_NODE_MONTHLY');
  assertEquals(priceEnvVar('advanced-automata', 'annual'), 'STRIPE_PRICE_ADVANCED_AUTOMATA_ANNUAL');
  assertEquals(
    priceEnvVar('global-enterprise', 'monthly'),
    'STRIPE_PRICE_GLOBAL_ENTERPRISE_MONTHLY',
  );
});

Deno.test('parseCheckoutRequest accepts a valid body', () => {
  const req = parseCheckoutRequest({
    companyName: '  Acme  ',
    billingEmail: 'billing@acme.co',
    industrySlug: 'automotive',
    tier: 'advanced-automata',
    billingCycle: 'annual',
    roster: [{ name: 'Ani', email: 'ani@acme.co', role: 'manager', subRole: 'Production' }],
  });
  assertEquals(req.companyName, 'Acme');
  assertEquals(req.region, 'us-east');
  assertEquals(req.tier, 'advanced-automata');
});

Deno.test('parseCheckoutRequest rejects invalid input', () => {
  assertThrows(() => parseCheckoutRequest({ companyName: '' }));
  assertThrows(() =>
    parseCheckoutRequest({ companyName: 'X', billingEmail: 'nope', tier: 'standard-node' }),
  );
  assertThrows(() =>
    parseCheckoutRequest({
      companyName: 'X',
      billingEmail: 'a@b.co',
      tier: 'bogus',
      billingCycle: 'monthly',
      industrySlug: 'automotive',
      roster: [{ name: 'A', email: 'a@b.co', role: 'employee', subRole: '' }],
    }),
  );
  // Empty roster is rejected.
  assertThrows(() =>
    parseCheckoutRequest({
      companyName: 'X',
      billingEmail: 'a@b.co',
      tier: 'standard-node',
      billingCycle: 'monthly',
      industrySlug: 'automotive',
      roster: [],
    }),
  );
});

Deno.test('buildCheckoutForm produces a subscription session body', () => {
  const form = buildCheckoutForm({
    priceId: 'price_123',
    intentId: 'intent_abc',
    billingEmail: 'a@b.co',
    successUrl: 'https://app/ok',
    cancelUrl: 'https://app/cancel',
  });
  assertEquals(form.mode, 'subscription');
  assertEquals(form['line_items[0][price]'], 'price_123');
  assertEquals(form['metadata[signup_intent_id]'], 'intent_abc');
});
