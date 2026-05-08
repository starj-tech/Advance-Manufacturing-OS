import { z } from 'zod';

/** UUID v4/v7 — accepted server-side. */
export const Uuid = z.string().uuid();
export type Uuid = z.infer<typeof Uuid>;

/** Hybrid Logical Clock timestamp serialized as `wall.logical.node`. */
export const Hlc = z.string().regex(/^\d+\.\d+\.[A-Za-z0-9_-]+$/, 'invalid HLC');
export type Hlc = z.infer<typeof Hlc>;

/** Tenant identifier. */
export const TenantId = Uuid.brand<'TenantId'>();
export type TenantId = z.infer<typeof TenantId>;

/** User identifier. */
export const UserId = Uuid.brand<'UserId'>();
export type UserId = z.infer<typeof UserId>;

/** ULID/UUIDv7-style identifier. */
export const EntityId = z.string().min(1).max(64);
export type EntityId = z.infer<typeof EntityId>;

/** Roles for the Dynamic Shell. */
export const Role = z.enum(['developer', 'executive', 'manager', 'employee']);
export type Role = z.infer<typeof Role>;

/** Permission scope, dotted form: `resource:action`. */
export const PermissionScope = z.string().regex(/^[a-z_]+:[a-z_*]+$/);
export type PermissionScope = z.infer<typeof PermissionScope>;
