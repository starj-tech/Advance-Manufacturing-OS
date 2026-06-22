import { z } from 'zod';
import { Hlc, TenantId, Uuid } from './primitives';

export const Material = z.object({
  id: Uuid,
  tenantId: TenantId,
  sku: z.string().min(1).max(64),
  /** Encrypted name surfaced as base64 ciphertext on the wire. */
  nameCiphertext: z.string().nullable(),
  nameBlindIdx: z.string().nullable(),
  uom: z.string().max(16),
  qtyOnHand: z.number(),
  qtyReserved: z.number(),
  hlc: Hlc,
});
export type Material = z.infer<typeof Material>;

/**
 * Inventory adjustments are NEVER plain UPDATEs on `qty_on_hand`.
 * Clients submit deltas via this envelope and the server enforces
 * `qty_on_hand + delta >= 0` atomically inside an RPC.
 */
export const InventoryAdjust = z.object({
  materialId: Uuid,
  delta: z.number(),
  reason: z.string().max(256),
  workOrderId: Uuid.nullable(),
});
export type InventoryAdjust = z.infer<typeof InventoryAdjust>;
