// Tests for list-tenants pure helpers. Run with `deno test`.
import { assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { toSummaries, userIdFromJwt } from './index.ts';

function jwt(payload: Record<string, unknown>): string {
  const b64 = (o: unknown) => btoa(JSON.stringify(o)).replace(/\+/g, '-').replace(/\//g, '_');
  return `${b64({ alg: 'HS256' })}.${b64(payload)}.sig`;
}

Deno.test('userIdFromJwt extracts sub', () => {
  assertEquals(userIdFromJwt(jwt({ sub: 'user-123' })), 'user-123');
  assertEquals(userIdFromJwt('garbage'), null);
  assertEquals(userIdFromJwt(jwt({})), null);
});

Deno.test('toSummaries joins tenants with their subscriptions', () => {
  const out = toSummaries(
    [
      {
        id: 't1',
        company_id: 'ACME-1',
        name: 'Acme',
        industry_slug: 'automotive',
        tier: 'advanced-automata',
        status: 'active',
      },
      {
        id: 't2',
        company_id: 'BETA-2',
        name: 'Beta',
        industry_slug: 'food-and-beverage',
        tier: 'standard-node',
        status: 'active',
      },
    ],
    [{ tenant_id: 't1', status: 'active', billing_cycle: 'annual' }],
  );
  assertEquals(out.length, 2);
  assertEquals(out[0]?.subscriptionStatus, 'active');
  assertEquals(out[0]?.billingCycle, 'annual');
  // t2 has no subscription row → 'none'.
  assertEquals(out[1]?.subscriptionStatus, 'none');
});
