import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type WorkOrderStatus } from '@aether/rpc-contracts';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { keys } from './query-keys';

export interface WorkOrderAdvanceInput {
  id: string;
  to: WorkOrderStatus;
  qtyDelta?: number;
  expectedHlc?: string;
  reason?: string;
}

export interface WorkOrderAdvanceResult {
  status: WorkOrderStatus;
  qtyDone: number;
  hlc: string;
}

/**
 * Atomic, permission-gated work-order state transition via the
 * work_order_advance RPC (migration 0020). Demo mode is a no-op; live mode
 * invalidates the work-orders cache on success so the table re-fetches.
 */
export function useWorkOrderAdvance() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useMutation({
    mutationFn: async (a: WorkOrderAdvanceInput): Promise<WorkOrderAdvanceResult | null> => {
      if (!supabase) return null;
      const { data, error } = await supabase.rpc('work_order_advance', {
        p_id: a.id,
        p_to: a.to,
        p_qty_delta: a.qtyDelta ?? 0,
        p_expected_hlc: a.expectedHlc ?? null,
        p_reason: a.reason || null,
      });
      if (error) throw new Error(error.message);
      const row = Array.isArray(data) ? data[0] : data;
      if (!row) return null;
      return {
        status: row.status as WorkOrderStatus,
        qtyDone: Number(row.qty_done ?? 0),
        hlc: String(row.hlc ?? ''),
      };
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.workOrders(tenantId) }),
  });
}
