import { z } from 'zod';
import { PermissionScope, Role } from './primitives';

/** Mirror of the TOML manifest format consumed by `aether-modules`. */
export const ModuleManifest = z.object({
  module: z.object({
    id: z.string().regex(/^[a-z0-9.-]+$/),
    name: z.string().min(1).max(128),
    version: z.string().regex(/^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$/),
    shells: z.array(Role).min(1),
    minAether: z
      .string()
      .regex(/^\d+\.\d+\.\d+$/)
      .optional(),
  }),
  entry: z.object({
    ui: z.string().min(1),
    backend: z.string().optional(),
  }),
  permissions: z.object({
    read: z.array(z.string()).default([]),
    write: z.array(z.string()).default([]),
    network: z.array(z.string()).default([]),
  }),
  dependencies: z.record(z.string()).default({}),
  signature: z.object({
    algorithm: z.literal('ed25519'),
    publicKeyId: z.string().min(1),
  }),
});
export type ModuleManifest = z.infer<typeof ModuleManifest>;

export const InstalledModule = z.object({
  id: z.string(),
  name: z.string(),
  version: z.string(),
  shells: z.array(Role),
  enabled: z.boolean(),
  policy: z.enum(['auto', 'manual', 'pinned']),
  pinnedVersion: z.string().nullable(),
  permissions: z.array(PermissionScope),
});
export type InstalledModule = z.infer<typeof InstalledModule>;
