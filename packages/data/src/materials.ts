import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { type InventoryAdjust, Material } from '@aether/rpc-contracts';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { keys } from './query-keys';

const MAT_COLUMNS =
  'id,tenant_id,sku,name_ciphertext,name_blind_idx,uom,qty_on_hand,qty_reserved,hlc';

function rowToMaterial(r: Record<string, unknown>): Record<string, unknown> {
  return {
    id: r.id,
    tenantId: r.tenant_id,
    sku: r.sku,
    nameCiphertext: r.name_ciphertext == null ? null : String(r.name_ciphertext),
    nameBlindIdx: r.name_blind_idx == null ? null : String(r.name_blind_idx),
    uom: r.uom ?? '',
    qtyOnHand: r.qty_on_hand == null ? 0 : Number(r.qty_on_hand),
    qtyReserved: r.qty_reserved == null ? 0 : Number(r.qty_reserved),
    hlc: r.hlc,
  };
}

const DEMO_TENANT = '00000000-0000-0000-0000-000000000001';

const DEMO_MATERIALS = Material.array().parse([
  {
    id: '0c000000-0000-4000-8000-000000000001',
    tenantId: DEMO_TENANT,
    sku: 'STR-CORE-3K',
    nameCiphertext: null,
    nameBlindIdx: null,
    uom: 'pcs',
    qtyOnHand: 320,
    qtyReserved: 80,
    hlc: '1.0.seed',
  },
  {
    id: '0c000000-0000-4000-8000-000000000002',
    tenantId: DEMO_TENANT,
    sku: 'RTR-SHAFT-12',
    nameCiphertext: null,
    nameBlindIdx: null,
    uom: 'pcs',
    qtyOnHand: 540,
    qtyReserved: 200,
    hlc: '1.0.seed',
  },
]);

/** Materials for the active tenant; demo data when no backend is configured. */
export function useMaterials() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: keys.materials(tenantId),
    queryFn: async (): Promise<Material[]> => {
      if (!supabase) return DEMO_MATERIALS;
      const { data, error } = await supabase
        .from('materials')
        .select(MAT_COLUMNS)
        .order('sku', { ascending: true });
      if (error) throw new Error(error.message);
      return Material.array().parse((data ?? []).map(rowToMaterial));
    },
    enabled: !supabase || tenantId.length > 0,
  });
}

/**
 * Atomic, permission-gated stock adjustment via the inventory_adjust RPC
 * (migration 0019). Demo mode is a no-op; live mode invalidates the materials
 * cache on success so the table re-fetches.
 */
export function useInventoryAdjust() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useMutation({
    mutationFn: async (a: InventoryAdjust): Promise<number | null> => {
      if (!supabase) return null;
      const { data, error } = await supabase.rpc('inventory_adjust', {
        p_material_id: a.materialId,
        p_delta: a.delta,
        p_reason: a.reason || null,
      });
      if (error) throw new Error(error.message);
      return typeof data === 'number' ? data : Number(data);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.materials(tenantId) }),
  });
}
