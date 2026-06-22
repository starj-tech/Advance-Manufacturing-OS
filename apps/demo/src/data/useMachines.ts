import { useQuery } from '@tanstack/react-query';
import { Machine } from '@aether/rpc-contracts';
import { supabase } from '@aether/supabase';
import { useSession } from '../core/session-provider';
import { keys } from './query-keys';
import { MACHINE_COLUMNS, rowToMachine } from './mappers';
import { DEMO_MACHINES } from './demo/machines';

/**
 * Machines for the active tenant. Returns demo data when no backend is
 * configured (supabase === null), otherwise queries Supabase and validates
 * every row against the Machine contract.
 */
export function useMachines() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';

  return useQuery({
    queryKey: keys.machines(tenantId),
    queryFn: async (): Promise<Machine[]> => {
      if (!supabase) return DEMO_MACHINES;
      const { data, error } = await supabase
        .from('machines')
        .select(MACHINE_COLUMNS)
        .order('code', { ascending: true });
      if (error) throw new Error(error.message);
      return Machine.array().parse((data ?? []).map(rowToMachine));
    },
    // In live mode, wait until the session supplies a tenant; in demo mode
    // (no backend) run immediately.
    enabled: !supabase || tenantId.length > 0,
  });
}
