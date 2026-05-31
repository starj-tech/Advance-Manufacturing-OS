// Tests for complete-password-change pure helpers. Run with `deno test`.
import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { callerFromJwt, strongEnough } from './index.ts';

function jwt(payload: Record<string, unknown>): string {
  const b64 = (o: unknown) => btoa(JSON.stringify(o)).replace(/\+/g, '-').replace(/\//g, '_');
  return `${b64({ alg: 'HS256' })}.${b64(payload)}.sig`;
}

Deno.test('strongEnough: length + letter + digit', () => {
  assert(!strongEnough(''));
  assert(!strongEnough('short1'));
  assert(!strongEnough('onlyletters'));
  assert(!strongEnough('1234567890'));
  assert(strongEnough('LongEnough1'));
});

Deno.test('callerFromJwt extracts sub + tenant', () => {
  const c = callerFromJwt(jwt({ sub: 'u-1', app_metadata: { tenant_id: 't-1' } }));
  assertEquals(c?.userId, 'u-1');
  assertEquals(c?.tenantId, 't-1');

  const noTenant = callerFromJwt(jwt({ sub: 'u-2' }));
  assertEquals(noTenant?.tenantId, null);

  assertEquals(callerFromJwt(jwt({})), null);
  assertEquals(callerFromJwt('garbage'), null);
});
