import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { useRealtimeInvalidate } from './realtime';

export interface AuditEvent {
  id: number;
  actorId: string | null;
  action: string;
  resource: string | null;
  resourceId: string | null;
  metadata: Record<string, unknown> | null;
  createdAt: string;
}

const DEMO_EVENTS: AuditEvent[] = [
  {
    id: 3,
    actorId: 'demo-manager',
    action: 'inventory_adjust',
    resource: 'materials',
    resourceId: 'STR-CORE-3K',
    metadata: { delta: -10, reason: 'kanban draw', new_qty: 310 },
    createdAt: '2026-05-31T08:42:00Z',
  },
  {
    id: 2,
    actorId: 'demo-manager',
    action: 'work_order.released',
    resource: 'work_orders',
    resourceId: 'WO-2026-0043',
    metadata: { from: 'draft', to: 'released' },
    createdAt: '2026-05-31T07:15:00Z',
  },
  {
    id: 1,
    actorId: 'demo-executive',
    action: 'login',
    resource: 'session',
    resourceId: null,
    metadata: null,
    createdAt: '2026-05-31T07:00:00Z',
  },
];

function rowToEvent(r: Record<string, unknown>): AuditEvent {
  const meta = r.metadata;
  return {
    id: Number(r.id),
    actorId: r.actor_id == null ? null : String(r.actor_id),
    action: String(r.action ?? ''),
    resource: r.resource == null ? null : String(r.resource),
    resourceId: r.resource_id == null ? null : String(r.resource_id),
    metadata: meta && typeof meta === 'object' ? (meta as Record<string, unknown>) : null,
    createdAt: String(r.created_at ?? ''),
  };
}

/**
 * Most recent audit-log rows for the active tenant. RLS scopes them to the
 * caller's tenant; demo mode returns canned events including an inventory_adjust
 * so the UI shows the expected shape with no backend.
 */
export function useAuditLog(limit = 50) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  useRealtimeInvalidate({
    table: 'audit_log',
    filter: tenantId ? `tenant_id=eq.${tenantId}` : undefined,
    invalidate: [['audit-log', tenantId, limit]],
  });
  return useQuery({
    queryKey: ['audit-log', tenantId, limit],
    queryFn: async (): Promise<AuditEvent[]> => {
      if (!supabase) return DEMO_EVENTS;
      const { data, error } = await supabase
        .from('audit_log')
        .select('id,actor_id,action,resource,resource_id,metadata,created_at')
        .order('created_at', { ascending: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToEvent);
    },
    enabled: !supabase || tenantId.length > 0,
    // Realtime is the fast path; safety-net poll every 2 minutes.
    refetchInterval: 120_000,
  });
}
