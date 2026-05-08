import type { Role } from '@aether/rpc-contracts';
import type { HostApi } from './host-api';

export interface ModuleContext {
  /** Mount point provided by the host shell. */
  container: HTMLElement;
  /** Capability-scoped host API. Calls outside granted permissions throw. */
  host: HostApi;
  /** Active shell when the module is mounted. */
  shell: Role;
  /** AbortSignal that fires when the host unmounts the module. */
  signal: AbortSignal;
}

export type MountFn = (ctx: ModuleContext) => void | (() => void);

export interface ModuleDefinition {
  id: string;
  version: string;
  shells: ReadonlyArray<Role>;
  mount: MountFn;
}

/**
 * Author entry-point for module bundles.
 *
 * @example
 *   export default defineModule({
 *     id: 'com.aether.qc-spc',
 *     version: '1.4.2',
 *     shells: ['manager'],
 *     mount({ container, host }) {
 *       container.textContent = 'SPC chart';
 *       const sub = host.read('telemetry').subscribe('press_01.temp', console.log);
 *       return () => sub.unsubscribe();
 *     },
 *   });
 */
export function defineModule(def: ModuleDefinition): ModuleDefinition {
  return def;
}
