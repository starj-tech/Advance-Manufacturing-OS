import type { RosterEntry } from './roster';
import type { BillingCycle, TierSlug } from './tiers';

export interface CheckoutPayload {
  companyName: string;
  billingEmail: string;
  region: string;
  industrySlug: string;
  tier: TierSlug;
  billingCycle: BillingCycle;
  roster: RosterEntry[];
}

const url = import.meta.env.VITE_SUPABASE_URL;
const anon = import.meta.env.VITE_SUPABASE_ANON_KEY;

/** True when the create-checkout Edge Function is reachable (backend configured). */
export const checkoutConfigured = Boolean(url && anon);

/**
 * Start Stripe Checkout via the create-checkout Edge Function. Returns the
 * hosted checkout URL, or `null` in demo mode (no backend) so the wizard can
 * show its review summary instead.
 */
export async function startCheckout(
  payload: CheckoutPayload,
): Promise<{ checkoutUrl: string } | null> {
  if (!url || !anon) return null;
  const res = await fetch(`${url}/functions/v1/create-checkout`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${anon}`,
      apikey: anon,
    },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`checkout failed: ${res.status}`);
  return (await res.json()) as { checkoutUrl: string };
}
