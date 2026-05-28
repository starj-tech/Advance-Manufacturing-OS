import { useQuery } from '@tanstack/react-query';
import { Machine, WorkOrder } from '@aether/rpc-contracts';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { capabilitiesFor } from '@aether/industry-catalog';
import { keys } from './query-keys';
import { MACHINE_COLUMNS, WO_COLUMNS, rowToMachine, rowToWorkOrder } from './mappers';
import { DEMO_MACHINES, DEMO_WORK_ORDERS } from './demo';

/** Machines for the active tenant; demo data when no backend is configured. */
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
    enabled: !supabase || tenantId.length > 0,
  });
}

/** Work orders for the active tenant; demo data when no backend is configured. */
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

/**
 * The active tenant's industry slug + resolved capabilities. Reads
 * tenant_industry from Supabase and expands the slug via the shared catalog;
 * falls back to Food & Beverage in demo mode so capability gating still shows.
 */
export function useTenantCapabilities() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: keys.tenantIndustry(tenantId),
    queryFn: async (): Promise<{ slug: string; capabilities: readonly string[] }> => {
      if (!supabase) {
        return { slug: 'food-and-beverage', capabilities: capabilitiesFor('food-and-beverage') };
      }
      const { data, error } = await supabase
        .from('tenant_industry')
        .select('industry_slug')
        .limit(1)
        .maybeSingle();
      if (error) throw new Error(error.message);
      const slug = (data?.industry_slug as string | undefined) ?? '';
      return { slug, capabilities: capabilitiesFor(slug) };
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
