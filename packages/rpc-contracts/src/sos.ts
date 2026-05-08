import { z } from 'zod';
import { TenantId, Uuid } from './primitives';

export const SosStatus = z.enum(['active', 'acknowledged', 'resolved', 'false-alarm']);
export type SosStatus = z.infer<typeof SosStatus>;

export const SosEvent = z.object({
  id: Uuid,
  tenantId: TenantId,
  userId: Uuid,
  triggeredAt: z.string().datetime(),
  locationLat: z.number().min(-90).max(90).nullable(),
  locationLng: z.number().min(-180).max(180).nullable(),
  locationAccuracyM: z.number().int().nonnegative().nullable(),
  status: SosStatus,
  acknowledgedBy: Uuid.nullable(),
  acknowledgedAt: z.string().datetime().nullable(),
  note: z.string().max(512).nullable(),
});
export type SosEvent = z.infer<typeof SosEvent>;

export const Geofence = z.object({
  id: Uuid,
  tenantId: TenantId,
  name: z.string().min(1).max(128),
  polygonGeoJson: z.unknown(),
  allowedBssids: z.array(z.string()),
  allowedBleBeacons: z.array(z.string()),
});
export type Geofence = z.infer<typeof Geofence>;
