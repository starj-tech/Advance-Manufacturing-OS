import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export interface Lot {
  id: string;
  lotCode: string;
  materialSku: string | null;
  description: string | null;
  qty: number | null;
  uom: string | null;
  madeAt: string;
  expiresAt: string | null;
  parentLotId: string | null;
}

const DEMO_LOTS: Lot[] = [
  {
    id: 'l1',
    lotCode: 'RAW-MILK-2026-0501',
    materialSku: 'MILK-RAW',
    description: 'Susu mentah 2,000 L (supplier A)',
    qty: 2000,
    uom: 'L',
    madeAt: new Date(Date.now() - 4 * 86_400_000).toISOString(),
    expiresAt: new Date(Date.now() + 2 * 86_400_000).toISOString(),
    parentLotId: null,
  },
  {
    id: 'l2',
    lotCode: 'RAW-SUGAR-2026-0419',
    materialSku: 'SUGAR-CANE',
    description: 'Gula pasir 500 kg',
    qty: 500,
    uom: 'kg',
    madeAt: new Date(Date.now() - 7 * 86_400_000).toISOString(),
    expiresAt: new Date(Date.now() + 180 * 86_400_000).toISOString(),
    parentLotId: null,
  },
  {
    id: 'l3',
    lotCode: 'MIX-YOG-2026-0503-A',
    materialSku: 'YOG-MIX',
    description: 'Adonan yogurt batch A',
    qty: 900,
    uom: 'L',
    madeAt: new Date(Date.now() - 2 * 86_400_000).toISOString(),
    expiresAt: new Date(Date.now() + 8 * 86_400_000).toISOString(),
    parentLotId: 'l1',
  },
  {
    id: 'l4',
    lotCode: 'YOG-STRAW-2026-0504',
    materialSku: 'YOG-FG',
    description: 'Yogurt rasa stroberi (FG)',
    qty: 1800,
    uom: 'cup',
    madeAt: new Date(Date.now() - 86_400_000).toISOString(),
    expiresAt: new Date(Date.now() + 14 * 86_400_000).toISOString(),
    parentLotId: 'l3',
  },
  {
    id: 'l5',
    lotCode: 'YOG-PLAIN-2026-0504',
    materialSku: 'YOG-FG',
    description: 'Yogurt plain (FG)',
    qty: 1500,
    uom: 'cup',
    madeAt: new Date(Date.now() - 86_400_000).toISOString(),
    expiresAt: new Date(Date.now() + 14 * 86_400_000).toISOString(),
    parentLotId: 'l3',
  },
];

function rowToLot(r: Record<string, unknown>): Lot {
  return {
    id: String(r.id ?? ''),
    lotCode: String(r.lot_code ?? ''),
    materialSku: r.material_sku == null ? null : String(r.material_sku),
    description: r.description == null ? null : String(r.description),
    qty: r.qty == null ? null : Number(r.qty),
    uom: r.uom == null ? null : String(r.uom),
    madeAt: String(r.made_at ?? new Date().toISOString()),
    expiresAt: r.expires_at == null ? null : String(r.expires_at),
    parentLotId: r.parent_lot_id == null ? null : String(r.parent_lot_id),
  };
}

/**
 * Lots for the active tenant; demo lineage when no backend is configured.
 * Live mode reads from the lots table (migration 0023). Genealogy is the
 * self-ref parent_lot_id chain.
 */
export function useLots(limit = 200) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['lots', tenantId, limit],
    queryFn: async (): Promise<Lot[]> => {
      if (!supabase) return DEMO_LOTS;
      const { data, error } = await supabase
        .from('lots')
        .select('id,lot_code,material_sku,description,qty,uom,made_at,expires_at,parent_lot_id')
        .order('made_at', { ascending: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToLot);
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
