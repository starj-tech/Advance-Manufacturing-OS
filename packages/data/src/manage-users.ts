import { useMutation, useQueryClient } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export interface ResetPasswordResult {
  ok: boolean;
  /** Temporary password the IT admin hands off to the user — visible once. */
  password?: string;
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
