import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export type ModuleStatus = 'published' | 'disabled' | 'deprecated';
export type TenantModulePolicy = 'auto' | 'manual' | 'pinned';

export interface TenantModule {
  moduleId: string;
  currentVersion: string;
  pinnedVersion: string | null;
  policy: TenantModulePolicy;
  enabled: boolean;
  status: ModuleStatus;
}

const DEMO_MODULES: TenantModule[] = [
  {
    moduleId: 'co.aether.cold-chain-monitor',
    currentVersion: '1.4.0',
    pinnedVersion: '1.4.0',
    policy: 'pinned',
    enabled: true,
    status: 'published',
  },
  {
    moduleId: 'co.aether.lot-genealogy',
    currentVersion: '0.9.2',
    pinnedVersion: null,
    policy: 'auto',
    enabled: true,
    status: 'published',
  },
  {
    moduleId: 'co.aether.predictive-maint',
    currentVersion: '2.1.0',
    pinnedVersion: null,
    policy: 'manual',
    enabled: false,
    status: 'published',
  },
];

function rowToTenantModule(r: Record<string, unknown>): TenantModule {
  const mod = (r.modules ?? {}) as Record<string, unknown>;
  return {
    moduleId: String(r.module_id ?? ''),
    currentVersion: String(mod.current_version ?? ''),
    pinnedVersion: r.pinned_version == null ? null : String(r.pinned_version),
    policy: (r.policy as TenantModulePolicy) ?? 'manual',
    enabled: r.enabled === true,
    status: (mod.status as ModuleStatus) ?? 'published',
  };
}

/** Installed modules for the active tenant (with the upstream module status). */
export function useTenantModules() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['tenant-modules', tenantId],
    queryFn: async (): Promise<TenantModule[]> => {
      if (!supabase) return DEMO_MODULES;
      const { data, error } = await supabase
        .from('tenant_modules')
        .select('module_id,pinned_version,policy,enabled,modules(current_version,status)')
        .order('module_id', { ascending: true });
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToTenantModule);
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
