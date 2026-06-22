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

/**
 * Forced first-login flow: calls the complete-password-change Edge Function
 * which atomically sets the new password AND clears must_change_password in
 * app_metadata (which a normal user JWT cannot do). After success, callers
 * MUST refresh the session so the new claims take effect.
 */
export async function completePasswordChange(
  newPassword: string,
): Promise<{ ok: boolean; error?: string }> {
  if (!supabase) return { ok: true };
  const { data } = await supabase.auth.getSession();
  const token = data.session?.access_token;
  if (!token) return { ok: false, error: 'not signed in' };
  const url = (import.meta as unknown as { env?: { VITE_SUPABASE_URL?: string } }).env
    ?.VITE_SUPABASE_URL;
  if (!url) return { ok: false, error: 'supabase url missing' };
  const res = await fetch(`${url}/functions/v1/complete-password-change`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ newPassword }),
  });
  if (!res.ok) {
    const e = (await res.json().catch(() => ({}))) as { error?: string };
    return { ok: false, error: e.error ?? `http ${res.status}` };
  }
  // Refresh so the new app_metadata (must_change_password: false) propagates.
  await supabase.auth.refreshSession();
  return { ok: true };
}
