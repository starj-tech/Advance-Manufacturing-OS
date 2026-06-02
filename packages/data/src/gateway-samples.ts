import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { useRealtimeInvalidate } from './realtime';

export interface GatewaySample {
  id: number;
  machineId: string;
  tag: string;
  value: unknown;
  takenAt: string;
  gatewayId: string | null;
}

const DEMO_SAMPLES: GatewaySample[] = (() => {
  const out: GatewaySample[] = [];
  const now = Date.now();
  for (let i = 0; i < 30; i++) {
    out.push({
      id: i + 1,
      machineId: i % 2 === 0 ? 'm-press-01' : 'm-cnc-12',
      tag: i % 2 === 0 ? 'press.force_kN' : 'cnc.spindle_rpm',
      value: i % 2 === 0 ? 240 + Math.round(Math.sin(i / 3) * 8) : 8500 + i * 12,
      takenAt: new Date(now - i * 5_000).toISOString(),
      gatewayId: 'gw-demo',
    });
  }
  return out;
})();

function rowToSample(r: Record<string, unknown>): GatewaySample {
  return {
    id: Number(r.id ?? 0),
    machineId: String(r.machine_id ?? ''),
    tag: String(r.tag ?? ''),
    value: r.value ?? null,
    takenAt: String(r.taken_at ?? new Date().toISOString()),
    gatewayId: r.gateway_id == null ? null : String(r.gateway_id),
  };
}

/**
 * Latest telemetry samples ingested by the on-prem gateway. RLS scopes to
 * the active tenant + the `devices:read` scope. Live mode subscribes to
 * Postgres CDC; demo mode returns a deterministic stream so the IT app's
 * Telemetri tab is never empty.
 */
export function useGatewaySamples(limit = 100) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  useRealtimeInvalidate({
    table: 'gateway_samples',
    filter: tenantId ? `tenant_id=eq.${tenantId}` : undefined,
    invalidate: [['gateway-samples', tenantId, limit]],
  });
  return useQuery({
    queryKey: ['gateway-samples', tenantId, limit],
    queryFn: async (): Promise<GatewaySample[]> => {
      if (!supabase) return DEMO_SAMPLES;
      const { data, error } = await supabase
        .from('gateway_samples')
        .select('id,machine_id,tag,value,taken_at,gateway_id')
        .order('taken_at', { ascending: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToSample);
    },
    enabled: !supabase || tenantId.length > 0,
    refetchInterval: 60_000,
  });
}
