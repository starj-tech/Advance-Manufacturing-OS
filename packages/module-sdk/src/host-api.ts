/**
 * Capability-gated API exposed by the host runtime to a loaded module.
 *
 * Each method maps to a capability declared in the module's manifest.
 * Calls outside the granted set are rejected at the IPC boundary by
 * `apps/desktop/src/modules/runtime.ts`.
 */

export interface Subscription {
  unsubscribe: () => void;
}

export interface ReadCapability {
  query<T = unknown>(filter?: Record<string, unknown>): Promise<T[]>;
  subscribe<T = unknown>(key: string, handler: (value: T) => void): Subscription;
}

export interface WriteCapability {
  upsert(payload: unknown): Promise<void>;
  remove(id: string): Promise<void>;
}

export interface HostApi {
  /** Read namespace, scoped by `permissions.read` in manifest. */
  read(namespace: string): ReadCapability;
  /** Write namespace, scoped by `permissions.write`. */
  write(namespace: string): WriteCapability;
  /** Network egress, scoped by `permissions.network` (host whitelist). */
  fetch(url: string, init?: RequestInit): Promise<Response>;
  /** Mount a glove-friendly modal/panel on the host shell. */
  prompt(args: { title: string; description?: string }): Promise<boolean>;
  /** Emit a toast/notification through the shared chrome. */
  notify(args: { kind: 'info' | 'warn' | 'error'; message: string }): void;
}
