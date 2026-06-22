import type { ReactNode } from 'react';
import type { PermissionScope } from '@aether/rpc-contracts';

/**
 * Pure, server-mirroring permission check. Caller passes in the user's
 * permission set (typically read from a JWT custom claim).
 *
 * NOTE: client-side checks are UX-only. Server-side RLS + RPC checks
 * are the authoritative gate.
 */
export function hasPermission(
  granted: ReadonlyArray<string>,
  scope: PermissionScope | string,
): boolean {
  if (granted.includes('*')) return true;
  if (granted.includes(scope)) return true;
  // Wildcard prefix support: `work_orders:*` matches `work_orders:approve`.
  for (const g of granted) {
    if (g.endsWith(':*')) {
      const prefix = g.slice(0, -1);
      if (scope.startsWith(prefix)) return true;
    }
  }
  return false;
}

export function usePermission(
  granted: ReadonlyArray<string>,
  scope: PermissionScope | string,
): boolean {
  return hasPermission(granted, scope);
}

export interface PermissionGateProps {
  granted: ReadonlyArray<string>;
  scope: PermissionScope | string;
  fallback?: ReactNode;
  children: ReactNode;
}

export function PermissionGate({ granted, scope, fallback = null, children }: PermissionGateProps) {
  return hasPermission(granted, scope) ? <>{children}</> : <>{fallback}</>;
}
