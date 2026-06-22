// AETHER-OS — mint-session edge function
//
// Called immediately after passkey login. Reads the user's tenant_users +
// permissions and returns a signed session payload that the client uses
// to populate `app_metadata` on the JWT (via Supabase Auth admin update).
//
// Skeleton: the real implementation in PR #2 wires this to Supabase Auth
// admin API. For now it returns a static payload so the desktop app can
// proceed past the login screen during dev.

// deno-lint-ignore-file no-explicit-any
import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

interface MintBody {
  userId: string;
  tenantId: string;
}

serve(async (req: Request) => {
  if (req.method !== 'POST') {
    return new Response('method not allowed', { status: 405 });
  }

  let body: MintBody;
  try {
    body = (await req.json()) as MintBody;
  } catch {
    return new Response('invalid body', { status: 400 });
  }

  // PR #2: query tenant_users + permissions, then call
  // supabaseAdmin.auth.admin.updateUserById(body.userId, { app_metadata: {...} }).

  return new Response(
    JSON.stringify({
      ok: true,
      claim: {
        tenant_id: body.tenantId,
        primary_role: 'manager',
        permissions: ['work_orders:*', 'machines:read', 'inventory:read'],
      },
      shipsIn: 'PR #2',
    }),
    { headers: { 'content-type': 'application/json' } },
  );
});
