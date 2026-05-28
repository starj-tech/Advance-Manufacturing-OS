import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import type { Role } from '@aether/shell-runtime';
import type { Session as SupabaseAuthSession } from '@supabase/supabase-js';
import { supabase, isSupabaseConfigured } from '@aether/supabase';

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
  /** Which backend the session came from: real Supabase auth or demo mode. */
  backend: 'supabase' | 'demo';
  /** Demo-mode helper: seeds a local session from a role picker. */
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

const VALID_ROLES: ReadonlyArray<Role> = ['developer', 'executive', 'manager', 'employee'];

/**
 * Maps a Supabase auth session to our Session shape. Role, tenant, and
 * permissions live in JWT `app_metadata`, populated server-side by the
 * mint-session Edge Function ({ tenant_id, primary_role, permissions }).
 */
function sessionFromSupabase(auth: SupabaseAuthSession): Session {
  const meta = (auth.user.app_metadata ?? {}) as Record<string, unknown>;
  const claimedRole = meta.primary_role;
  const primaryRole: Role =
    typeof claimedRole === 'string' && (VALID_ROLES as string[]).includes(claimedRole)
      ? (claimedRole as Role)
      : 'employee'; // least-privilege default
  const permissions = Array.isArray(meta.permissions)
    ? meta.permissions.filter((p): p is string => typeof p === 'string')
    : [];
  const userMeta = (auth.user.user_metadata ?? {}) as Record<string, unknown>;
  const displayName =
    (typeof userMeta.full_name === 'string' && userMeta.full_name) || auth.user.email || 'User';
  return {
    userId: auth.user.id,
    tenantId: typeof meta.tenant_id === 'string' ? meta.tenant_id : '',
    primaryRole,
    permissions,
    displayName,
  };
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!supabase) {
      // Demo mode: no backend configured. Resolve immediately and let the
      // login screen seed a dev-mode role.
      setLoading(false);
      return;
    }

    let active = true;
    void supabase.auth.getSession().then(({ data }) => {
      if (!active) return;
      setSession(data.session ? sessionFromSupabase(data.session) : null);
      setLoading(false);
    });
    const { data: sub } = supabase.auth.onAuthStateChange((_event, next) => {
      setSession(next ? sessionFromSupabase(next) : null);
    });
    return () => {
      active = false;
      sub.subscription.unsubscribe();
    };
  }, []);

  const value = useMemo<SessionContextValue>(
    () => ({
      session,
      loading,
      backend: isSupabaseConfigured ? 'supabase' : 'demo',
      signInDevMode: (role) => {
        setSession({
          userId: `dev-${role}`,
          tenantId: DEV_TENANT_ID,
          primaryRole: role,
          permissions: DEV_PERMISSIONS[role],
          displayName: `${role[0]?.toUpperCase()}${role.slice(1)} (dev)`,
        });
      },
      signOut: () => {
        if (supabase) void supabase.auth.signOut();
        setSession(null);
      },
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
