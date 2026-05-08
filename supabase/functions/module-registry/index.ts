// AETHER-OS — module-registry edge function
//
// List published modules visible to the caller's tenant. Returns module
// IDs, current versions, and signature metadata. The bundle bytes
// themselves are served from Supabase Storage; this function only
// catalogs.
//
// Skeleton: returns an empty list. PR #4 implements the registry query
// and signature transparency log integration.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

serve(async () => {
  return new Response(
    JSON.stringify({
      modules: [],
      shipsIn: 'PR #4',
    }),
    { headers: { 'content-type': 'application/json' } },
  );
});
