// AETHER-OS — sos-escalation edge function
//
// Triggered by a Postgres webhook on INSERT into sos_events. Fans out to
// SMS (Twilio), Slack, email, and an optional security-system webhook.
//
// Skeleton: validates payload shape and acknowledges. Real escalation
// targets are wired in PR #5 once tenant config (notification routes)
// schema lands.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

interface SosWebhookPayload {
  type: 'INSERT';
  table: 'sos_events';
  record: {
    id: string;
    tenant_id: string;
    user_id: string;
    triggered_at: string;
    location_lat: number | null;
    location_lng: number | null;
    note: string | null;
  };
}

serve(async (req: Request) => {
  if (req.method !== 'POST') {
    return new Response('method not allowed', { status: 405 });
  }

  let payload: SosWebhookPayload;
  try {
    payload = (await req.json()) as SosWebhookPayload;
  } catch {
    return new Response('invalid body', { status: 400 });
  }

  if (payload.type !== 'INSERT' || payload.table !== 'sos_events') {
    return new Response('not interested', { status: 200 });
  }

  console.warn(`[sos-escalation] event ${payload.record.id} tenant=${payload.record.tenant_id}`);

  // PR #5: read tenant escalation_routes, fan-out to Twilio/Slack/email.

  return new Response(JSON.stringify({ ok: true, fanout: 0, shipsIn: 'PR #5' }), {
    headers: { 'content-type': 'application/json' },
  });
});
