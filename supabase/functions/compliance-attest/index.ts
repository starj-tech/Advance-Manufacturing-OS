// AETHER-OS — compliance-attest edge function.
//
// Receives a finalized, hash-chained ComplianceReport (produced by
// `aether_compliance::ProbeRunner`), RE-VERIFIES the chain server-side
// using the exact same byte layout as the Rust `runner::next_hash`, and —
// only if the chain is intact — returns an Ed25519 signature over the
// report's final chain hash. The signature is persisted in
// `compliance_reports.attestation_signature`; external auditors verify a
// report against the platform's published public key without trusting us.
//
// ## Hash layout (must match crates/aether-compliance/src/runner.rs)
//   hash[i] = SHA-256( prev_hash
//                    || u32_le(len(control_id)) || control_id_utf8
//                    || verdict_byte
//                    || u32_le(len(evidence))  || evidence_utf8 )
//   verdict_byte: pass=1, fail=2, not-applicable=3, needs-review=4
//   hash[-1] = 32 zero bytes. The report digest is the last entry's hash.
// A cross-language golden vector pins this (runner.rs
// `golden_chain_hash_is_stable` + the Deno test here).
//
// ## Key management
// The Ed25519 private key (PKCS#8 DER, base64) comes from the
// ATTESTATION_SIGNING_KEY env var; the matching public key (base64) from
// ATTESTATION_PUBLIC_KEY, returned so callers can pin it. Missing key =>
// 500 (deploy misconfig), never a silent unsigned response.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

export type VerdictSlug = 'pass' | 'fail' | 'not-applicable' | 'needs-review';

export interface ChainedVerdict {
  control_point_id: string;
  verdict: VerdictSlug;
  evidence: string;
  prev_hash: number[];
  hash: number[];
}

export interface ComplianceReport {
  id: string;
  tenant_id: string;
  standard_slug: string;
  generated_at: string;
  status: string;
  controls: ChainedVerdict[];
}

export const VERDICT_BYTE: Record<VerdictSlug, number> = {
  pass: 1,
  fail: 2,
  'not-applicable': 3,
  'needs-review': 4,
};

const HASH_LEN = 32;

function u32le(n: number): Uint8Array {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n >>> 0, true);
  return b;
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((s, p) => s + p.length, 0);
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
  return diff === 0;
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

const enc = new TextEncoder();

/** One chain step. Mirrors `runner::next_hash` byte-for-byte. */
export async function nextHash(
  prev: Uint8Array,
  controlId: string,
  verdict: VerdictSlug,
  evidence: string,
): Promise<Uint8Array> {
  const id = enc.encode(controlId);
  const ev = enc.encode(evidence);
  const vb = VERDICT_BYTE[verdict];
  if (vb === undefined) throw new Error(`unknown verdict: ${verdict}`);
  const msg = concat([prev, u32le(id.length), id, new Uint8Array([vb]), u32le(ev.length), ev]);
  const digest = await crypto.subtle.digest('SHA-256', msg);
  return new Uint8Array(digest);
}

export interface VerifyResult {
  ok: boolean;
  /** Index of the first bad entry, when !ok. */
  badIndex?: number;
  reason?: string;
  /** Final chain hash (last entry's hash), when ok and non-empty. */
  finalHash?: Uint8Array;
}

/** Re-walk and validate the hash chain exactly as `ComplianceReport::verify_chain` does. */
export async function verifyChain(controls: ChainedVerdict[]): Promise<VerifyResult> {
  let expectedPrev = new Uint8Array(HASH_LEN);
  let last: Uint8Array | undefined;
  for (let i = 0; i < controls.length; i++) {
    const e = controls[i];
    const prev = Uint8Array.from(e.prev_hash ?? []);
    if (!bytesEqual(prev, expectedPrev)) {
      return { ok: false, badIndex: i, reason: 'prev_hash mismatch' };
    }
    const computed = await nextHash(expectedPrev, e.control_point_id, e.verdict, e.evidence);
    const stored = Uint8Array.from(e.hash ?? []);
    if (!bytesEqual(stored, computed)) {
      return { ok: false, badIndex: i, reason: 'hash mismatch' };
    }
    expectedPrev = computed;
    last = computed;
  }
  return { ok: true, finalHash: last };
}

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function bytesToBase64(bytes: Uint8Array): string {
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** Sign 32 raw digest bytes with the env-configured Ed25519 key. */
export async function signDigest(digest: Uint8Array): Promise<string> {
  const keyB64 = Deno.env.get('ATTESTATION_SIGNING_KEY');
  if (!keyB64) throw new Error('ATTESTATION_SIGNING_KEY not configured');
  const key = await crypto.subtle.importKey(
    'pkcs8',
    base64ToBytes(keyB64),
    { name: 'Ed25519' },
    false,
    ['sign'],
  );
  const sig = await crypto.subtle.sign({ name: 'Ed25519' }, key, digest);
  return bytesToBase64(new Uint8Array(sig));
}

export async function handler(req: Request): Promise<Response> {
  const json = (body: unknown, status: number) =>
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    });

  if (req.method !== 'POST') return json({ error: 'method_not_allowed' }, 405);

  let report: ComplianceReport;
  try {
    report = (await req.json()) as ComplianceReport;
  } catch {
    return json({ error: 'invalid_json' }, 400);
  }
  if (!report?.tenant_id || !report?.standard_slug || !Array.isArray(report.controls)) {
    return json({ error: 'missing_fields' }, 400);
  }
  if (report.controls.length === 0) {
    return json({ error: 'empty_report', detail: 'no controls to attest' }, 400);
  }

  let verdict: VerifyResult;
  try {
    verdict = await verifyChain(report.controls);
  } catch (e) {
    return json({ error: 'verify_failed', detail: String(e) }, 400);
  }
  if (!verdict.ok || !verdict.finalHash) {
    return json(
      { error: 'chain_invalid', badIndex: verdict.badIndex, reason: verdict.reason },
      422,
    );
  }

  let signature: string;
  try {
    signature = await signDigest(verdict.finalHash);
  } catch (e) {
    // Missing/invalid key is a deploy misconfiguration, not client error.
    return json({ error: 'signing_unavailable', detail: String(e) }, 500);
  }

  return json(
    {
      algorithm: 'ed25519',
      reportId: report.id,
      tenantId: report.tenant_id,
      standardSlug: report.standard_slug,
      status: report.status,
      digest: toHex(verdict.finalHash),
      signature,
      publicKey: Deno.env.get('ATTESTATION_PUBLIC_KEY') ?? null,
    },
    200,
  );
}

// Only start the server when run as the entrypoint, so tests can import
// the pure helpers without binding a port.
if (import.meta.main) {
  serve(handler);
}
