import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';
import { useRealtimeInvalidate } from './realtime';

export interface TemperatureReading {
  id: number;
  sensorId: string;
  sensorLabel: string;
  takenAt: string;
  celsius: number;
  lowC: number | null;
  highC: number | null;
}

const DEMO_READINGS: TemperatureReading[] = (() => {
  const out: TemperatureReading[] = [];
  const sensors: Array<[string, string, number, number, number]> = [
    ['COOL-A', 'Cold room A (susu)', 4, 2, 6],
    ['COOL-B', 'Cold room B (yoghurt)', 3.5, 2, 5],
    ['FREEZ-1', 'Freezer 1 (es krim)', -19, -22, -16],
  ];
  const now = Date.now();
  for (const [id, label, base, low, high] of sensors) {
    for (let mins = 0; mins <= 60; mins += 5) {
      const drift = id === 'COOL-A' && mins < 15 ? 1.6 : 0;
      out.push({
        id: out.length + 1,
        sensorId: id,
        sensorLabel: label,
        takenAt: new Date(now - mins * 60_000).toISOString(),
        celsius: Number((base + drift + Math.sin(mins / 7) * 0.4).toFixed(2)),
        lowC: low,
        highC: high,
      });
    }
  }
  return out;
})();

function rowToReading(r: Record<string, unknown>): TemperatureReading {
  return {
    id: Number(r.id ?? 0),
    sensorId: String(r.sensor_id ?? ''),
    sensorLabel: r.sensor_label == null ? '' : String(r.sensor_label),
    takenAt: String(r.taken_at ?? new Date().toISOString()),
    celsius: Number(r.celsius ?? 0),
    lowC: r.low_c == null ? null : Number(r.low_c),
    highC: r.high_c == null ? null : Number(r.high_c),
  };
}

/**
 * Recent temperature readings across all sensors for the active tenant.
 * Live mode reads from temperature_readings (migration 0022), demo mode
 * synthesizes a deterministic-ish curve so the chart looks alive.
 */
export function useColdChainReadings(limit = 240) {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  useRealtimeInvalidate({
    table: 'temperature_readings',
    filter: tenantId ? `tenant_id=eq.${tenantId}` : undefined,
    invalidate: [['cold-chain-readings', tenantId, limit]],
  });
  return useQuery({
    queryKey: ['cold-chain-readings', tenantId, limit],
    queryFn: async (): Promise<TemperatureReading[]> => {
      if (!supabase) return DEMO_READINGS;
      const { data, error } = await supabase
        .from('temperature_readings')
        .select('id,sensor_id,sensor_label,taken_at,celsius,low_c,high_c')
        .order('taken_at', { ascending: false })
        .limit(limit);
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToReading);
    },
    enabled: !supabase || tenantId.length > 0,
    // Realtime is the fast path; this is just a safety net if the WS drops.
    refetchInterval: 120_000,
  });
}
