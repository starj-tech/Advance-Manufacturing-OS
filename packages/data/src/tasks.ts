import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export type TaskStatus = 'todo' | 'doing' | 'done';
export type TaskPriority = 'low' | 'normal' | 'high' | 'urgent';

export interface ShopFloorTask {
  id: string;
  title: string;
  instruction: string | null;
  assignedTo: string | null;
  priority: TaskPriority;
  status: TaskStatus;
  createdAt: string;
  dueAt: string | null;
}

const DEMO_TASKS: ShopFloorTask[] = [
  {
    id: 't1',
    title: 'Cek suhu COOL-A',
    instruction: 'Verifikasi suhu Cold room A; jika di atas 6°C koordinasi dengan maintenance.',
    assignedTo: null,
    priority: 'urgent',
    status: 'todo',
    createdAt: new Date().toISOString(),
    dueAt: new Date(Date.now() + 30 * 60_000).toISOString(),
  },
  {
    id: 't2',
    title: 'Mulai WO-2026-0043',
    instruction: 'Persiapkan bahan baku susu untuk produksi yogurt batch B.',
    assignedTo: null,
    priority: 'high',
    status: 'todo',
    createdAt: new Date().toISOString(),
    dueAt: new Date(Date.now() + 2 * 3_600_000).toISOString(),
  },
  {
    id: 't3',
    title: 'Verifikasi inventory STR-CORE-3K',
    instruction: 'Stock-take cycle: hitung fisik vs sistem; lapor selisih.',
    assignedTo: null,
    priority: 'normal',
    status: 'doing',
    createdAt: new Date().toISOString(),
    dueAt: new Date(Date.now() + 6 * 3_600_000).toISOString(),
  },
];

function rowToTask(r: Record<string, unknown>): ShopFloorTask {
  return {
    id: String(r.id ?? ''),
    title: String(r.title ?? ''),
    instruction: r.instruction == null ? null : String(r.instruction),
    assignedTo: r.assigned_to == null ? null : String(r.assigned_to),
    priority: (r.priority as TaskPriority) ?? 'normal',
    status: (r.status as TaskStatus) ?? 'todo',
    createdAt: String(r.created_at ?? new Date().toISOString()),
    dueAt: r.due_at == null ? null : String(r.due_at),
  };
}

/** Shop-floor tasks visible to the active user (RLS filters by tenant + perm). */
export function useShopFloorTasks(limit = 50) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['shop-floor-tasks', tenantId, limit],
    queryFn: async (): Promise<ShopFloorTask[]> => {
      if (!supabase) return DEMO_TASKS;
      const { data, error } = await supabase
        .from('shop_floor_tasks')
        .select('id,title,instruction,assigned_to,priority,status,created_at,due_at')
        .order('status', { ascending: true })
        .order('due_at', { ascending: true, nullsFirst: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToTask);
    },
    enabled: !supabase || tenantId.length > 0,
    refetchInterval: 20_000,
  });
}

/** Move a task between todo / doing / done. RLS gates this to the assignee. */
export function useUpdateTaskStatus() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useMutation({
    mutationFn: async (args: { id: string; status: TaskStatus }): Promise<void> => {
      if (!supabase) return;
      const { error } = await supabase
        .from('shop_floor_tasks')
        .update({ status: args.status })
        .eq('id', args.id);
      if (error) throw new Error(error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ['shop-floor-tasks', tenantId] }),
  });
}
