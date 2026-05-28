/**
 * AETHER-OS Module SDK.
 *
 * Vendors author modules against this SDK. Modules are loaded at runtime
 * inside a Web Worker by `apps/demo/src/modules/runtime.ts`. The host
 * exposes a capability-gated API matching the `permissions` declared in
 * the module's manifest (TOML); attempts to call beyond the granted set
 * are rejected at the worker message boundary.
 */

export type { Role, ModuleManifest, InstalledModule } from '@aether/rpc-contracts';
export { defineModule } from './define-module';
export type { ModuleDefinition, ModuleContext, MountFn } from './define-module';
export type { HostApi, ReadCapability, WriteCapability } from './host-api';
