// Tests for send-welcome-email pure helper. Run with `deno test`.
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { buildWelcomeEmail } from './index.ts';

Deno.test('buildWelcomeEmail includes company id, tier, and every username', () => {
  const email = buildWelcomeEmail({
    companyName: 'PT Contoh',
    companyId: 'CONTOH-7Q2F',
    tier: 'advanced-automata',
    billingEmail: 'billing@contoh.co',
    accounts: [
      { name: 'Ani', username: 'ani', role: 'manager' },
      { name: 'Budi', username: 'budi', role: 'employee' },
    ],
  });
  assertStringIncludes(email.subject, 'PT Contoh');
  assertStringIncludes(email.html, 'CONTOH-7Q2F');
  assertStringIncludes(email.html, 'advanced-automata');
  assertStringIncludes(email.html, 'ani');
  assertStringIncludes(email.html, 'budi');
  assertStringIncludes(email.text, 'CONTOH-7Q2F');
});

Deno.test('buildWelcomeEmail escapes HTML in names', () => {
  const email = buildWelcomeEmail({
    companyName: 'A & B <Co>',
    companyId: 'AB-1234',
    tier: 'standard-node',
    billingEmail: 'a@b.co',
    accounts: [{ name: '<script>', username: 'x', role: 'it' }],
  });
  assert(!email.html.includes('<script>'));
  assertStringIncludes(email.html, '&lt;script&gt;');
  assertEquals(email.subject.includes('&'), true);
});
