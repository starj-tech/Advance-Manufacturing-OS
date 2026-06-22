import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { summarize, userIdFromJwt } from './index.ts';

function jwt(payload: Record<string, unknown>): string {
  const b64 = (o: unknown) => btoa(JSON.stringify(o)).replace(/\+/g, '-').replace(/\//g, '_');
  return `${b64({ alg: 'HS256' })}.${b64(payload)}.sig`;
}

Deno.test('userIdFromJwt extracts sub', () => {
  assertEquals(userIdFromJwt(jwt({ sub: 'u1' })), 'u1');
  assertEquals(userIdFromJwt('garbage'), null);
});

Deno.test('summarize counts installs + kill-switches', () => {
  const out = summarize(
    [
      { id: 'mod.a', current_version: '1.0', status: 'published' },
      { id: 'mod.b', current_version: '0.2', status: 'deprecated' },
    ],
    [
      { module_id: 'mod.a', enabled: true },
      { module_id: 'mod.a', enabled: true },
      { module_id: 'mod.a', enabled: false },
      { module_id: 'mod.b', enabled: true },
    ],
  );
  assertEquals(out.length, 2);
  const a = out.find((m) => m.id === 'mod.a')!;
  assertEquals(a.installs, 3);
  assertEquals(a.enabledInstalls, 2);
  const b = out.find((m) => m.id === 'mod.b')!;
  assertEquals(b.installs, 1);
  assert(b.status === 'deprecated');
});
