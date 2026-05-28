import { supabase } from '@aether/supabase';

/**
 * Map Company-ID + Username onto the synthetic Supabase Auth email. This MUST
 * stay identical to the provisioning side (stripe-webhook `syntheticEmail`),
 * or a provisioned user can't sign in. Both lowercase username + company id.
 */
export function syntheticEmail(username: string, companyId: string): string {
  return `${username.toLowerCase()}@${companyId.toLowerCase()}.tenant.aether-os.internal`;
}

export interface CompanyCredentials {
  companyId: string;
  username: string;
  password: string;
}

/** Sign in with Company ID + Username + password. Throws in demo mode. */
export async function signInWithCompanyId(creds: CompanyCredentials) {
  if (!supabase) throw new Error('backend not configured');
  return supabase.auth.signInWithPassword({
    email: syntheticEmail(creds.username, creds.companyId),
    password: creds.password,
  });
}

/**
 * Plain email + password sign-in (used by the vendor Console, where staff are
 * platform_admins rather than tenant users). Throws in demo mode.
 */
export async function signInWithEmail(email: string, password: string) {
  if (!supabase) throw new Error('backend not configured');
  return supabase.auth.signInWithPassword({ email, password });
}

/** Set a new password for the signed-in user (first-login reset). */
export async function changePassword(newPassword: string) {
  if (!supabase) throw new Error('backend not configured');
  return supabase.auth.updateUser({ password: newPassword });
}
