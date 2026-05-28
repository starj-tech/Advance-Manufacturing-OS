import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import type { Role } from '@aether/rpc-contracts';
import type { Session as SupabaseAuthSession } from '@supabase/supabase-js';
import { isSupabaseConfigured, supabase } from '@aether/supabase';

export interface Session {
  userId: string;
  tenantId: string;
  companyId: string;
  primaryRole: Role;
  subRole: string;
  permissions: readonly string[];
  displayName: string;
  /** True until the user resets their generated password on first login. */
  mustChangePassword: boolean;
}

interface SessionContextValue {
  session: Session | null;
  loading: boolean;
  backend: 'supabase' | 'demo';
  signOut: () => void;
}

const SessionContext = createContext<SessionContextValue | null>(null);

const VALID_ROLES: ReadonlyArray<Role> = ['developer', 'executive', 'manager', 'employee', 'it'];

/**
 * Map a Supabase auth session to our Session. Claims live in JWT app_metadata,
 * populated by mint-session / the stripe-webhook provisioner:
 * { tenant_id, company_id, primary_role, sub_role, permissions, must_change_password }.
 */
function sessionFromSupabase(auth: SupabaseAuthSession): Session {
  const meta = (auth.user.app_metadata ?? {}) as Record<string, unknown>;
  const claimedRole = meta.primary_role;
  const primaryRole: Role =
    typeof claimedRole === 'string' && (VALID_ROLES as string[]).includes(claimedRole)
      ? (claimedRole as Role)
      : 'employee';
  const permissions = Array.isArray(meta.permissions)
    ? meta.permissions.filter((p): p is string => typeof p === 'string')
    : [];
  const userMeta = (auth.user.user_metadata ?? {}) as Record<string, unknown>;
  const displayName =
    (typeof userMeta.full_name === 'string' && userMeta.full_name) || auth.user.email || 'User';
  return {
    userId: auth.user.id,
    tenantId: typeof meta.tenant_id === 'string' ? meta.tenant_id : '',
    companyId: typeof meta.company_id === 'string' ? meta.company_id : '',
    primaryRole,
    subRole: typeof meta.sub_role === 'string' ? meta.sub_role : '',
    permissions,
    displayName,
    mustChangePassword: meta.must_change_password === true,
  };
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!supabase) {
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
  if (!ctx) throw new Error('useSession must be used within <SessionProvider>');
  return ctx;
}
