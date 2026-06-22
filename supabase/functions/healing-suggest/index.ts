// AETHER-OS — healing-suggest edge function
//
// Receives a Symptom payload from a client's Neural Auto-Healing engine
// and returns a suggested HealingPolicy plus rationale. This is the
// "AI" tier in the engine's chain: when local rule-based healers can't
// reach a confident diagnosis, the symptom is forwarded here for an
// LLM-assisted classification (PR #5 wires Anthropic Claude with prompt
// caching).
//
// Skeleton: validates the payload shape and returns 501.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

interface Symptom {
  source: string;
  kind: string;
  detail: unknown;
  severity: number;
}

serve(async (req: Request) => {
  if (req.method !== 'POST') {
    return new Response('method not allowed', { status: 405 });
  }
  let s: Symptom;
  try {
    s = (await req.json()) as Symptom;
  } catch {
    return new Response('invalid body', { status: 400 });
  }
  if (typeof s.source !== 'string' || typeof s.kind !== 'string') {
    return new Response('invalid symptom', { status: 400 });
  }
  console.warn(`[healing-suggest] symptom ${s.source}/${s.kind} severity=${s.severity}`);
  return new Response(
    JSON.stringify({
      error: 'not_implemented',
      shipsIn: 'PR #5 — neural healing dispatcher',
    }),
    { status: 501, headers: { 'content-type': 'application/json' } },
  );
});
