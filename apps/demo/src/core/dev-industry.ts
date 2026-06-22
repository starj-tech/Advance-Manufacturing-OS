/**
 * Development-time industry assignment for the Demo app.
 *
 * In the Main app, the active industry + capabilities come from the tenant's
 * `tenant_industry` row in Supabase, resolved against @aether/industry-catalog.
 * The Demo hard-codes Food & Beverage so the Manager Shell Inventory/Machines
 * pages can demonstrate Industry-Specific Logic Injection (expiry dates,
 * cold-chain pills, etc.) end-to-end with zero backend.
 *
 * Swap the slug to preview another vertical; capabilities come from the
 * shared catalog so the Demo never drifts from the real profile data.
 */
import { capabilitiesFor } from '@aether/industry-catalog';

export const DEV_INDUSTRY_SLUG = 'food-and-beverage';

export const DEV_INDUSTRY_CAPABILITIES: ReadonlyArray<string> = capabilitiesFor(DEV_INDUSTRY_SLUG);
