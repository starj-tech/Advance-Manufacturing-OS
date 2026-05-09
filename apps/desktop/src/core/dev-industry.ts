/**
 * Development-time industry assignment.
 *
 * In production, the active industry + capabilities come from the
 * `industry_profile` Tauri command (which calls
 * `aether-industry::profile_for(industry)` against the persisted
 * `tenant_industry` row). PR #6 wires that flow.
 *
 * For the skeleton we hard-code Food & Beverage so the Manager Shell
 * Inventory and Machines pages can demonstrate Industry-Specific Logic
 * Injection (expiry dates, cold-chain pills, etc.) end-to-end.
 *
 * To preview a different industry profile during dev, swap the slug +
 * capability list below. The full catalog lives in
 * `crates/aether-industry/src/profile.rs::profile_for`.
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
