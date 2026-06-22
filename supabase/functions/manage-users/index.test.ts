// Tests for manage-users pure helpers. Run with `deno test`.
import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import {
  authorizeIt,
  callerFromJwt,
  generatePassword,
  parseRosterCsv,
  usernameFor,
} from './index.ts';

function jwt(appMetadata: Record<string, unknown>, sub?: string): string {
  const b64 = (o: unknown) => btoa(JSON.stringify(o)).replace(/\+/g, '-').replace(/\//g, '_');
  const payload = sub ? { sub, app_metadata: appMetadata } : { app_metadata: appMetadata };
  return `${b64({ alg: 'HS256' })}.${b64(payload)}.sig`;
}

Deno.test('parseRosterCsv parses + skips header, normalizes role', () => {
  const rows = parseRosterCsv(
    'name,email,role,sub_role\nAni,ani@a.co,manager,Production\nX,x@a.co,wizard,',
  );
  assertEquals(rows.length, 2);
  assertEquals(rows[0]?.role, 'manager');
  assertEquals(rows[1]?.role, 'employee'); // unknown → employee
});

Deno.test('usernameFor derives from email local-part', () => {
  assertEquals(
    usernameFor({ name: 'Ani A', email: 'ani.a@acme.co', role: 'manager', subRole: '' }),
    'ani.a',
  );
  assertEquals(
    usernameFor({ name: 'No Email', email: '', role: 'employee', subRole: '' }),
    'no.email',
  );
});

Deno.test('generatePassword length + charset', () => {
  const pw = generatePassword(20);
  assertEquals(pw.length, 20);
  assert(/^[A-Za-z0-9]+$/.test(pw));
});

Deno.test('callerFromJwt + authorizeIt gate on the it role', () => {
  const itCaller = callerFromJwt(jwt({ primary_role: 'it', tenant_id: 't1' }, 'user-1'));
  assertEquals(itCaller?.role, 'it');
  assertEquals(itCaller?.userId, 'user-1');
  assert(authorizeIt(itCaller));
  assert(!authorizeIt(callerFromJwt(jwt({ primary_role: 'manager', tenant_id: 't1' }))));
  assert(!authorizeIt(callerFromJwt('garbage')));
  assert(!authorizeIt(callerFromJwt(jwt({ tenant_id: 't1' })))); // missing role
});

Deno.test('callerFromJwt tolerates missing sub claim', () => {
  const c = callerFromJwt(jwt({ primary_role: 'it', tenant_id: 't1' }));
  assertEquals(c?.userId, null);
});
