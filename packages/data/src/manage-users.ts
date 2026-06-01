import { useMutation, useQueryClient } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export interface ResetPasswordResult {
  ok: boolean;
  /** Temporary password the IT admin hands off to the user — visible once. */
  password?: string;
  error?: string;
}

export interface ImportRosterResult {
  ok: boolean;
  /** Per-user credentials shown ONCE so IT can hand them off securely. */
  created: Array<{ username: string; role: string; password: string }>;
  skipped: Array<{ username: string; reason: string }>;
  error?: string;
}

/**
 * Asks the manage-users Edge Function to reset a tenant member's password. The
 * function verifies the caller has the `it` role and writes an audit_log row.
 * Demo mode returns a fake password without touching anything.
 */
export function useResetUserPassword() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useMutation({
    mutationFn: async (userId: string): Promise<ResetPasswordResult> => {
      if (!supabase) {
        return { ok: true, password: 'DemoModeNoReset' };
      }
      const { data, error } = await supabase.functions.invoke('manage-users', {
        body: { action: 'reset-password', userId },
      });
      if (error) return { ok: false, error: error.message };
      const r = (data ?? {}) as ResetPasswordResult;
      return r;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tenant-users', tenantId] }),
  });
}

/**
 * Bulk-create accounts from a CSV roster via manage-users (import-roster).
 * Returns generated credentials ONCE so the IT admin can hand them off; demo
 * mode returns an empty list without touching anything.
 */
export function useImportRoster() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  const companyId = session?.companyId ?? '';
  return useMutation({
    mutationFn: async (csv: string): Promise<ImportRosterResult> => {
      if (!supabase) return { ok: true, created: [], skipped: [] };
      const { data, error } = await supabase.functions.invoke('manage-users', {
        body: { action: 'import-roster', csv, companyId },
      });
      if (error) return { ok: false, created: [], skipped: [], error: error.message };
      const r = (data ?? {}) as ImportRosterResult;
      return r;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tenant-users', tenantId] }),
  });
}
