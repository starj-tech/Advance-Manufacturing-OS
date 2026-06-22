// Tests for set-tenant-status pure helpers. Run with `deno test`.
import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { userIdFromJwt, validStatus } from './index.ts';

function jwt(payload: Record<string, unknown>): string {
  const b64 = (o: unknown) => btoa(JSON.stringify(o)).replace(/\+/g, '-').replace(/\//g, '_');
  return `${b64({ alg: 'HS256' })}.${b64(payload)}.sig`;
}

Deno.test('validStatus only accepts active | suspended', () => {
  assert(validStatus('active'));
  assert(validStatus('suspended'));
  assert(!validStatus('deleted'));
  assert(!validStatus(''));
  assert(!validStatus(null));
  assert(!validStatus(42));
});

Deno.test('userIdFromJwt extracts sub claim', () => {
  assertEquals(userIdFromJwt(jwt({ sub: 'user-7' })), 'user-7');
  assertEquals(userIdFromJwt(jwt({})), null);
  assertEquals(userIdFromJwt('not.a.jwt.too.many'), null);
  assertEquals(userIdFromJwt('garbage'), null);
});
