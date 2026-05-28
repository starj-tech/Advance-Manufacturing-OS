/**
 * Development-time industry assignment.
 *
 * In production, the active industry + capabilities come from the
 * tenant's `tenant_industry` row in Supabase (resolved against the
 * industry-profile catalog server-side).
 *
 * For local dev we hard-code Food & Beverage so the Manager Shell
 * Inventory and Machines pages can demonstrate Industry-Specific Logic
 * Injection (expiry dates, cold-chain pills, etc.) end-to-end.
 *
 * To preview a different industry profile during dev, swap the slug +
 * capability list below.
 */

export const DEV_INDUSTRY_SLUG = 'food-and-beverage';

export const DEV_INDUSTRY_CAPABILITIES: ReadonlyArray<string> = [
  // Food & Beverage profile (mirror of profile_for(FoodAndBeverage)).
  'expired-date-tracking',
  'cold-chain-monitor',
  'temperature-sensor-integration',
  'haccp-checks',
  'allergen-control',
  'recall-readiness',
];
