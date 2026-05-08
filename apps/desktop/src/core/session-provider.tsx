import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import type { Role } from '@aether/shell-runtime';

export interface Session {
  userId: string;
  tenantId: string;
  primaryRole: Role;
  permissions: ReadonlyArray<string>;
  displayName: string;
}

interface SessionContextValue {
  session: Session | null;
  loading: boolean;
  /** Test-mode helper: lets the login screen seed a fake session in PR #1. */
  signInDevMode: (role: Role) => void;
  signOut: () => void;
}

const SessionContext = createContext<SessionContextValue | null>(null);

const DEV_PERMISSIONS: Record<Role, string[]> = {
  developer: ['*'],
  executive: ['kpi:read', 'twin:read', 'finance:read'],
  manager: [
    'work_orders:read',
    'work_orders:approve',
    'machines:read',
    'inventory:read',
    'maintenance:read',
  ],
  employee: ['tasks:read', 'tasks:complete', 'sos:trigger', 'clock:write'],
};

const DEV_TENANT_ID = '00000000-0000-0000-0000-000000000001';

export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // PR #2: real Supabase session restore happens here. For skeleton,
    // we resolve immediately with no session and let the user pick a
    // dev-mode role on the login screen.
    setLoading(false);
  }, []);

  const value = useMemo<SessionContextValue>(
    () => ({
      session,
      loading,
      signInDevMode: (role) => {
        setSession({
          userId: `dev-${role}`,
          tenantId: DEV_TENANT_ID,
          primaryRole: role,
          permissions: DEV_PERMISSIONS[role],
          displayName: `${role[0]?.toUpperCase()}${role.slice(1)} (dev)`,
        });
      },
      signOut: () => setSession(null),
    }),
    [session, loading],
  );

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionContextValue {
  const ctx = useContext(SessionContext);
  if (!ctx) {
    throw new Error('useSession must be used within <SessionProvider>');
  }
  return ctx;
}
