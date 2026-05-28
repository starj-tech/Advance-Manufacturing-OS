import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

/** A company member as shown in the IT app's account management. */
export interface TenantUser {
  userId: string;
  username: string;
  role: string;
  subRole: string;
  mustChangePassword: boolean;
}

const DEMO_USERS: TenantUser[] = [
  { userId: 'u1', username: 'ceo', role: 'executive', subRole: 'CEO', mustChangePassword: false },
  {
    userId: 'u2',
    username: 'prod.manager',
    role: 'manager',
    subRole: 'Production',
    mustChangePassword: false,
  },
  {
    userId: 'u3',
    username: 'qc.inspector',
    role: 'employee',
    subRole: 'QC Inspector',
    mustChangePassword: true,
  },
  {
    userId: 'u4',
    username: 'it.admin',
    role: 'it',
    subRole: 'IT Admin',
    mustChangePassword: false,
  },
];

function rowToTenantUser(r: Record<string, unknown>): TenantUser {
  return {
    userId: String(r.user_id ?? ''),
    username: String(r.username ?? ''),
    role: String(r.primary_role ?? ''),
    subRole: String(r.sub_role ?? ''),
    mustChangePassword: r.must_change_password === true,
  };
}

/** All members of the active tenant; demo roster when no backend is configured. */
export function useTenantUsers() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['tenant-users', tenantId],
    queryFn: async (): Promise<TenantUser[]> => {
      if (!supabase) return DEMO_USERS;
      const { data, error } = await supabase
        .from('tenant_users')
        .select('user_id,username,primary_role,sub_role,must_change_password')
        .order('username', { ascending: true });
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToTenantUser);
    },
    enabled: !supabase || tenantId.length > 0,
  });
}
