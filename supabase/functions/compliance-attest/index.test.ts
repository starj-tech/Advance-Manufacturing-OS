// Tests for compliance-attest. Run with `deno test --allow-env`.
//
// The golden hash is shared with the Rust side
// (crates/aether-compliance/src/runner.rs `golden_chain_hash_is_stable`).
// If either side changes the chain byte layout, both goldens must move
// together — that's the whole point of pinning it in two languages.

import { assert, assertEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import {
  type ChainedVerdict,
  handler,
  nextHash,
  signDigest,
  verifyChain,
  type VerdictSlug,
} from './index.ts';

const GOLDEN = '4ff29b8b640277d152545416a3c1a76f80d37ae420eff17b27ad0f3ccc262441';

function hex(b: Uint8Array): string {
  return Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
}

/** Build a valid chained report from (control_id, verdict, evidence) triples. */
async function chain(entries: Array<[string, VerdictSlug, string]>): Promise<ChainedVerdict[]> {
  const out: ChainedVerdict[] = [];
  let prev = new Uint8Array(32);
  for (const [id, verdict, evidence] of entries) {
    const h = await nextHash(prev, id, verdict, evidence);
    out.push({
      control_point_id: id,
      verdict,
      evidence,
      prev_hash: Array.from(prev),
      hash: Array.from(h),
    });
    prev = h;
  }
  return out;
}

Deno.test('nextHash matches the cross-language golden vector', async () => {
  const h0 = await nextHash(new Uint8Array(32), 'is-key-rotation', 'fail', 'overdue');
  const h1 = await nextHash(h0, 'fs-cold-chain', 'pass', 'ok');
  assertEquals(hex(h1), GOLDEN);
});

Deno.test('verifyChain accepts an intact chain and returns the final hash', async () => {
  const controls = await chain([
    ['is-key-rotation', 'fail', 'overdue'],
    ['fs-cold-chain', 'pass', 'ok'],
  ]);
  const r = await verifyChain(controls);
  assert(r.ok);
  assertEquals(hex(r.finalHash!), GOLDEN);
});

Deno.test('verifyChain rejects a tampered verdict', async () => {
  const controls = await chain([
    ['is-key-rotation', 'fail', 'overdue'],
    ['fs-cold-chain', 'pass', 'ok'],
  ]);
  controls[0].verdict = 'pass'; // flip without recomputing hashes
  const r = await verifyChain(controls);
  assert(!r.ok);
  assertEquals(r.badIndex, 0);
});

Deno.test('verifyChain rejects a reordered chain', async () => {
  const controls = await chain([
    ['a', 'pass', 'x'],
    ['b', 'fail', 'y'],
    ['c', 'pass', 'z'],
  ]);
  [controls[0], controls[1]] = [controls[1], controls[0]];
  const r = await verifyChain(controls);
  assert(!r.ok);
});

Deno.test('signDigest produces a signature the public key verifies', async () => {
  const kp = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
    'sign',
    'verify',
  ])) as CryptoKeyPair;
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey('pkcs8', kp.privateKey));
  let bin = '';
  for (const b of pkcs8) bin += String.fromCharCode(b);
  Deno.env.set('ATTESTATION_SIGNING_KEY', btoa(bin));

  const digest = await nextHash(new Uint8Array(32), 'is-key-rotation', 'fail', 'overdue');
  const sigB64 = await signDigest(digest);
  const sig = Uint8Array.from(atob(sigB64), (c) => c.charCodeAt(0));

  const ok = await crypto.subtle.verify({ name: 'Ed25519' }, kp.publicKey, sig, digest);
  assert(ok);
});

Deno.test('handler attests a valid report end to end', async () => {
  const kp = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
    'sign',
    'verify',
  ])) as CryptoKeyPair;
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey('pkcs8', kp.privateKey));
  let bin = '';
  for (const b of pkcs8) bin += String.fromCharCode(b);
  Deno.env.set('ATTESTATION_SIGNING_KEY', btoa(bin));

  const controls = await chain([
    ['is-key-rotation', 'fail', 'overdue'],
    ['fs-cold-chain', 'pass', 'ok'],
  ]);
  const report = {
    id: 'r1',
    tenant_id: 't1',
    standard_slug: 'iso-27001',
    generated_at: new Date().toISOString(),
    status: 'non-compliant',
    controls,
  };
  const res = await handler(
    new Request('http://x/compliance-attest', {
      method: 'POST',
      body: JSON.stringify(report),
    }),
  );
  assertEquals(res.status, 200);
  const out = await res.json();
  assertEquals(out.algorithm, 'ed25519');
  assertEquals(out.digest, GOLDEN);
  assert(typeof out.signature === 'string' && out.signature.length > 0);
});

Deno.test('handler rejects a tampered report with 422', async () => {
  const controls = await chain([['is-key-rotation', 'fail', 'overdue']]);
  controls[0].evidence = 'tampered';
  const res = await handler(
    new Request('http://x/compliance-attest', {
      method: 'POST',
      body: JSON.stringify({
        id: 'r1',
        tenant_id: 't1',
        standard_slug: 'iso-27001',
        generated_at: new Date().toISOString(),
        status: 'non-compliant',
        controls,
      }),
    }),
  );
  assertEquals(res.status, 422);
});
