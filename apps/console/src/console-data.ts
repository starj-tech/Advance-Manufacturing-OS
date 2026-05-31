import { useCallback, useEffect, useState } from 'react';
import { supabase } from '@aether/supabase';

export interface TenantSummary {
  id: string;
  companyId: string;
  name: string;
  industrySlug: string;
  tier: string;
  status: string;
  subscriptionStatus: string;
  billingCycle: string;
}

const DEMO_TENANTS: TenantSummary[] = [
  {
    id: 't1',
    companyId: 'ACME-7Q2F',
    name: 'Acme Motors',
    industrySlug: 'automotive',
    tier: 'advanced-automata',
    status: 'active',
    subscriptionStatus: 'active',
    billingCycle: 'annual',
  },
  {
    id: 't2',
    companyId: 'NUSA-3K8M',
    name: 'Nusantara Foods',
    industrySlug: 'food-and-beverage',
    tier: 'standard-node',
    status: 'active',
    subscriptionStatus: 'active',
    billingCycle: 'monthly',
  },
  {
    id: 't3',
    companyId: 'BIOME-X1',
    name: 'BioMed Pharma',
    industrySlug: 'pharmaceuticals',
    tier: 'global-enterprise',
    status: 'suspended',
    subscriptionStatus: 'past_due',
    billingCycle: 'annual',
  },
];

interface State {
  data: TenantSummary[];
  loading: boolean;
  error: boolean;
  /** True when serving the built-in demo list (no backend configured). */
  demo: boolean;
}

export type TenantStatus = 'active' | 'suspended';

export interface SetTenantStatusResult {
  ok: boolean;
  error?: string;
}

/**
 * Cross-tenant tenant list for the Console. Calls the service-role
 * list-tenants Edge Function (which gates on platform_admins); falls back to a
 * demo list when no backend is configured.
 */
export function useConsoleTenants(): State & { refresh: () => void } {
  const [state, setState] = useState<State>({ data: [], loading: true, error: false, demo: false });
  const [nonce, setNonce] = useState(0);
  const refresh = useCallback(() => setNonce((n) => n + 1), []);

  useEffect(() => {
    if (!supabase) {
      setState({ data: DEMO_TENANTS, loading: false, error: false, demo: true });
      return;
    }
    let active = true;
    void (async () => {
      try {
        const { data: sessionData } = await supabase.auth.getSession();
        const token = sessionData.session?.access_token;
        const url = (import.meta as unknown as { env: { VITE_SUPABASE_URL?: string } }).env
          .VITE_SUPABASE_URL;
        const res = await fetch(`${url}/functions/v1/list-tenants`, {
          headers: { authorization: `Bearer ${token ?? ''}` },
        });
        if (!res.ok) throw new Error(String(res.status));
        const body = (await res.json()) as { tenants: TenantSummary[] };
        if (active) setState({ data: body.tenants, loading: false, error: false, demo: false });
      } catch {
        if (active) setState({ data: [], loading: false, error: true, demo: false });
      }
    })();
    return () => {
      active = false;
    };
  }, [nonce]);

  return { ...state, refresh };
}

/**
 * Vendor-only action to flip a tenant between active and suspended. Calls the
 * set-tenant-status Edge Function (which gates on platform_admins and audits
 * to the target tenant's audit_log).
 */
export function useSetTenantStatus(): {
  setStatus: (
    tenantId: string,
    status: TenantStatus,
    reason?: string,
  ) => Promise<SetTenantStatusResult>;
  pending: boolean;
} {
  const [pending, setPending] = useState(false);

  const setStatus = useCallback(
    async (
      tenantId: string,
      status: TenantStatus,
      reason?: string,
    ): Promise<SetTenantStatusResult> => {
      if (!supabase) return { ok: true };
      setPending(true);
      try {
        const { data: sessionData } = await supabase.auth.getSession();
        const token = sessionData.session?.access_token;
        const url = (import.meta as unknown as { env: { VITE_SUPABASE_URL?: string } }).env
          .VITE_SUPABASE_URL;
        const res = await fetch(`${url}/functions/v1/set-tenant-status`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            authorization: `Bearer ${token ?? ''}`,
          },
          body: JSON.stringify({ tenantId, status, reason }),
        });
        if (!res.ok) {
          const err = (await res.json().catch(() => ({}))) as { error?: string };
          return { ok: false, error: err.error ?? `http ${res.status}` };
        }
        return { ok: true };
      } catch (e) {
        return { ok: false, error: (e as Error).message };
      } finally {
        setPending(false);
      }
    },
    [],
  );

  return { setStatus, pending };
}
