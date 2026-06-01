import { useQuery } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

export interface DeviceBinding {
  machineId: string;
  code: string;
  name: string;
  protocol: string | null;
  endpoint: string | null;
  nodeId: string | null;
  lastHeartbeat: string | null;
}

const DEMO_DEVICES: DeviceBinding[] = [
  {
    machineId: 'm1',
    code: 'PRESS-01',
    name: 'Hydraulic press 250t',
    protocol: 'opcua',
    endpoint: 'opc.tcp://10.0.0.21:4840',
    nodeId: 'ns=2;s=Press.State',
    lastHeartbeat: new Date(Date.now() - 60_000).toISOString(),
  },
  {
    machineId: 'm2',
    code: 'CNC-12',
    name: 'CNC mill A12',
    protocol: 'mqtt',
    endpoint: 'mqtt://10.0.0.34:1883/cnc/12',
    nodeId: 'cnc/12/state',
    lastHeartbeat: new Date(Date.now() - 5 * 60_000).toISOString(),
  },
];

function rowToBinding(r: Record<string, unknown>): DeviceBinding {
  const pb = (r.protocol_binding ?? {}) as Record<string, unknown>;
  return {
    machineId: String(r.id ?? ''),
    code: String(r.code ?? ''),
    name: String(r.name ?? ''),
    protocol: pb.protocol == null ? null : String(pb.protocol),
    endpoint: pb.endpoint == null ? null : String(pb.endpoint),
    nodeId: pb.node_id == null ? null : String(pb.node_id),
    lastHeartbeat: r.last_heartbeat == null ? null : String(r.last_heartbeat),
  };
}

/** Per-machine protocol bindings + last heartbeat for the IT app's device tab. */
export function useDeviceBindings() {
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useQuery({
    queryKey: ['device-bindings', tenantId],
    queryFn: async (): Promise<DeviceBinding[]> => {
      if (!supabase) return DEMO_DEVICES;
      const { data, error } = await supabase
        .from('machines')
        .select('id,code,name,protocol_binding,last_heartbeat')
        .order('code', { ascending: true });
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToBinding);
    },
    enabled: !supabase || tenantId.length > 0,
    refetchInterval: 30_000,
  });
}
