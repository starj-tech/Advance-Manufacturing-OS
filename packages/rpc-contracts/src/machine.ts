import { z } from 'zod';
import { Hlc, TenantId, Uuid } from './primitives';

export const MachineStatus = z.enum([
  'idle',
  'running',
  'paused',
  'fault',
  'maintenance',
  'offline',
]);
export type MachineStatus = z.infer<typeof MachineStatus>;

export const Machine = z.object({
  id: Uuid,
  tenantId: TenantId,
  code: z.string().min(1).max(64),
  name: z.string().min(1).max(256),
  protocolBinding: z
    .object({
      kind: z.enum(['opcua', 'mqtt', 'modbus']),
      endpoint: z.string(),
      tags: z.record(z.string()),
    })
    .nullable(),
  status: MachineStatus,
  lastHeartbeat: z.string().datetime().nullable(),
  hlc: Hlc,
});
export type Machine = z.infer<typeof Machine>;

export const TelemetrySample = z.object({
  ts: z.string().datetime(),
  tenantId: TenantId,
  machineId: Uuid,
  tag: z.string().min(1).max(128),
  value: z.number(),
  quality: z.number().int().min(0).max(255),
});
export type TelemetrySample = z.infer<typeof TelemetrySample>;
