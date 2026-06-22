// AETHER-OS — commodity-feed edge function
//
// Cron-driven (Supabase scheduled trigger every 5 minutes). Pulls latest
// quotes from configured market data providers (LME, CME, ICE, SHFE,
// platform-specific REST APIs gated by tenant API keys), normalizes to
// USD, and bulk-inserts into `commodity_prices`.
//
// Skeleton: validates payload shape and returns 501. Real provider
// adapters and rate-limit accounting land in PR #7.

import { serve } from 'https://deno.land/std@0.224.0/http/server.ts';

type Ticker = 'HRC' | 'ALI' | 'HG' | 'PP' | 'BZ' | 'NG' | 'LITH' | 'CO';

interface FeedTrigger {
  /** Optional whitelist; default = all tickers. */
  tickers?: Ticker[];
}

serve(async (req: Request) => {
  if (req.method !== 'POST') {
    return new Response('method not allowed', { status: 405 });
  }
  try {
    (await req.json()) as FeedTrigger;
  } catch {
    return new Response('invalid body', { status: 400 });
  }

  // PR #7:
  //   1. fetch tickers per provider with rate-limit budget
  //   2. convert to USD using FX snapshot
  //   3. INSERT INTO commodity_prices with conflict-do-update
  //   4. emit metric: commodity_feed.ingest.count

  return new Response(
    JSON.stringify({
      error: 'not_implemented',
      shipsIn: 'PR #7 — analytics module',
    }),
    { status: 501, headers: { 'content-type': 'application/json' } },
  );
});
