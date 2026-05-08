// AETHER-OS — compliance-attest edge function
//
// Receives a finalized ComplianceReport from the client, verifies that
// the underlying evidence hashes match what's stored in
// `compliance_evidence`, and returns an Ed25519 signature over the
// canonicalized payload. The signature is then persisted in
// `compliance_reports.attestation_signature` so external auditors can
// verify the report against the platform's public attestation key.
//
// Skeleton: validates the request shape and returns 501. Real signing
// flow + key management land in PR #6.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

interface AttestRequest {
  reportId: string;
  tenantId: string;
  standardSlug: string;
  status: 'compliant' | 'needs-review' | 'non-compliant';
  controlsHash: string; // sha256 over the canonicalized controls array
}

serve(async (req: Request) => {
  if (req.method !== 'POST') {
    return new Response('method not allowed', { status: 405 });
  }
  let body: AttestRequest;
  try {
    body = (await req.json()) as AttestRequest;
  } catch {
    return new Response('invalid body', { status: 400 });
  }
  if (!body.reportId || !body.tenantId || !body.standardSlug) {
    return new Response('missing fields', { status: 400 });
  }

  return new Response(
    JSON.stringify({
      error: 'not_implemented',
      shipsIn: 'PR #6 — compliance probes + attestation signing',
    }),
    { status: 501, headers: { 'content-type': 'application/json' } },
  );
});
