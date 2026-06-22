// Tests for stripe-webhook pure helpers. Run with `deno test --allow-env`.
import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import {
  companyId,
  generatePassword,
  planAccounts,
  slugifyCompany,
  syntheticEmail,
  verifyStripeSignature,
} from './index.ts';

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

async function stripeSig(payload: string, secret: string, t: number): Promise<string> {
  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const mac = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(`${t}.${payload}`));
  return hex(new Uint8Array(mac));
}

Deno.test('slugifyCompany + companyId', () => {
  assertEquals(slugifyCompany('PT Acme Tbk!!'), 'PTACMETB');
  assertEquals(companyId('PT Acme Tbk!!', 'AB12'), 'PTACMETB-AB12');
  assertEquals(slugifyCompany('***'), 'TENANT');
});

Deno.test('syntheticEmail maps username + company id', () => {
  assertEquals(syntheticEmail('ani', 'ACME-7Q2F'), 'ani@acme-7q2f.tenant.aether-os.internal');
});

Deno.test('planAccounts dedupes usernames and assigns passwords', () => {
  const accounts = planAccounts(
    [
      { name: 'Ani A', email: 'ani@a.co', role: 'manager', subRole: 'Production' },
      { name: 'Ani B', email: 'ani@b.co', role: 'employee', subRole: 'Operator' },
      { name: 'Budi', email: 'budi@a.co', role: 'employee', subRole: '' },
    ],
    () => 'FIXED-PW',
  );
  assertEquals(
    accounts.map((a) => a.username),
    ['ani', 'ani2', 'budi'],
  );
  assert(accounts.every((a) => a.password === 'FIXED-PW'));
});

Deno.test('generatePassword has the requested length and safe charset', () => {
  const pw = generatePassword(16);
  assertEquals(pw.length, 16);
  assert(/^[A-Za-z0-9]+$/.test(pw));
});

Deno.test('verifyStripeSignature accepts a valid signature and rejects tampering', async () => {
  const payload = '{"type":"checkout.session.completed"}';
  const secret = 'whsec_test';
  const t = 1_700_000_000;
  const sig = await stripeSig(payload, secret, t);
  assert(await verifyStripeSignature(payload, `t=${t},v1=${sig}`, secret, t));
  // Wrong body.
  assert(!(await verifyStripeSignature(payload + 'x', `t=${t},v1=${sig}`, secret, t)));
  // Outside tolerance.
  assert(!(await verifyStripeSignature(payload, `t=${t},v1=${sig}`, secret, t + 10_000)));
  // Missing v1.
  assert(!(await verifyStripeSignature(payload, `t=${t}`, secret, t)));
});
