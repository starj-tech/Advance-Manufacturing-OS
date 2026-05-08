// AETHER-OS — WebAuthn attestation verification.
//
// Validates passkey registration (attestationObject + clientDataJSON)
// and persists the credential id + public key for future assertions.
//
// Skeleton: returns 501 NotImplemented. PR #2 wires this against
// `@simplewebauthn/server` and the tenant_credentials table.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

serve(() => {
  return new Response(
    JSON.stringify({
      error: 'not_implemented',
      shipsIn: 'PR #2 — zero-knowledge auth layer',
    }),
    { status: 501, headers: { 'content-type': 'application/json' } },
  );
});
