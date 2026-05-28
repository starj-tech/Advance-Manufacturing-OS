import { useQuery } from '@tanstack/react-query';
import { WorkOrder } from '@aether/rpc-contracts';
import { supabase } from '../lib/supabase';
import { useSession } from '../core/session-provider';
import { keys } from './query-keys';
import { WO_COLUMNS, rowToWorkOrder } from './mappers';
import { DEMO_WORK_ORDERS } from './demo/work-orders';

/**
 * Work orders for the active tenant. Demo data when no backend is configured,
 * otherwise validated Supabase rows.
 */
export function useWorkOrders() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';

  return useQuery({
    queryKey: keys.workOrders(tenantId),
    queryFn: async (): Promise<WorkOrder[]> => {
      if (!supabase) return DEMO_WORK_ORDERS;
      const { data, error } = await supabase
        .from('work_orders')
        .select(WO_COLUMNS)
        .order('code', { ascending: true });
      if (error) throw new Error(error.message);
      return WorkOrder.array().parse((data ?? []).map(rowToWorkOrder));
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
