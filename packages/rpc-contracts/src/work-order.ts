import { z } from 'zod';
import { EntityId, Hlc, TenantId, Uuid } from './primitives';

export const WorkOrderStatus = z.enum([
  'draft',
  'released',
  'running',
  'paused',
  'completed',
  'canceled',
]);
export type WorkOrderStatus = z.infer<typeof WorkOrderStatus>;

export const WorkOrder = z.object({
  id: Uuid,
  tenantId: TenantId,
  code: z.string().min(1).max(64),
  machineId: Uuid.nullable(),
  status: WorkOrderStatus,
  qtyPlanned: z.number().nonnegative(),
  qtyDone: z.number().nonnegative(),
  dueAt: z.string().datetime().nullable(),
  createdBy: Uuid.nullable(),
  hlc: Hlc,
  parentHlc: Hlc.nullable(),
});
export type WorkOrder = z.infer<typeof WorkOrder>;

export const WorkOrderTransition = z.object({
  id: Uuid,
  from: WorkOrderStatus,
  to: WorkOrderStatus,
  /** Echoed back to the server to enforce optimistic concurrency. */
  expectedHlc: Hlc,
  reason: z.string().max(256).optional(),
});
export type WorkOrderTransition = z.infer<typeof WorkOrderTransition>;

/**
 * Outbox change envelope for the local-first sync engine.
 * `payload` is opaque to the wire — for encrypted entities it is ciphertext.
 */
export const SyncChange = z.object({
  opId: EntityId,
  entity: z.string(),
  entityId: EntityId,
  op: z.enum(['insert', 'update', 'delete', 'crdt_patch']),
  payload: z.unknown(),
  hlc: Hlc,
  parentHlc: Hlc.nullable(),
  encrypted: z.boolean(),
});
export type SyncChange = z.infer<typeof SyncChange>;
