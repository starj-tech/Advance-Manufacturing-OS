import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { supabase } from '@aether/supabase';
import { useSession } from '@aether/auth';

/**
 * Supported hardware integration paths. Half are mediated by the on-prem
 * gateway (opcua, mqtt, modbus-*, http-poll) — half can be driven directly
 * from the browser (web-serial, web-usb, web-bluetooth, web-hid, mqtt-ws,
 * websocket). The DB stores the binding config regardless; the runtime
 * driver is chosen at connect time based on the protocol slug.
 */
export const PROTOCOL_FAMILIES = [
  'opcua',
  'mqtt',
  'mqtt-ws',
  'modbus-tcp',
  'modbus-rtu',
  'http-poll',
  'websocket',
  'web-serial',
  'web-usb',
  'web-bluetooth',
  'web-hid',
  'manual',
  'file-upload',
] as const;

export type ProtocolFamily = (typeof PROTOCOL_FAMILIES)[number];

export type ProtocolPath = 'gateway' | 'browser-direct' | 'misc';

export const PROTOCOL_PATH: Record<ProtocolFamily, ProtocolPath> = {
  opcua: 'gateway',
  mqtt: 'gateway',
  'modbus-tcp': 'gateway',
  'modbus-rtu': 'gateway',
  'http-poll': 'gateway',
  'mqtt-ws': 'browser-direct',
  websocket: 'browser-direct',
  'web-serial': 'browser-direct',
  'web-usb': 'browser-direct',
  'web-bluetooth': 'browser-direct',
  'web-hid': 'browser-direct',
  manual: 'misc',
  'file-upload': 'misc',
};

export type MachineStatusLite = 'running' | 'idle' | 'paused' | 'fault' | 'maintenance' | 'offline';

export interface UpsertMachineInput {
  id?: string | null;
  code: string;
  name: string;
  status: MachineStatusLite;
  protocol: ProtocolFamily;
  binding: Record<string, unknown>;
}

export interface DeviceBinding {
  machineId: string;
  code: string;
  name: string;
  status: string;
  protocol: string | null;
  endpoint: string | null;
  nodeId: string | null;
  lastHeartbeat: string | null;
  binding: Record<string, unknown>;
}

const DEMO_DEVICES: DeviceBinding[] = [
  {
    machineId: 'm1',
    code: 'PRESS-01',
    name: 'Hydraulic press 250t',
    status: 'running',
    protocol: 'opcua',
    endpoint: 'opc.tcp://10.0.0.21:4840',
    nodeId: 'ns=2;s=Press.State',
    lastHeartbeat: new Date(Date.now() - 60_000).toISOString(),
    binding: {
      protocol: 'opcua',
      endpoint: 'opc.tcp://10.0.0.21:4840',
      node_id: 'ns=2;s=Press.State',
    },
  },
  {
    machineId: 'm2',
    code: 'CNC-12',
    name: 'CNC mill A12',
    status: 'idle',
    protocol: 'mqtt',
    endpoint: 'mqtt://10.0.0.34:1883/cnc/12',
    nodeId: 'cnc/12/state',
    lastHeartbeat: new Date(Date.now() - 5 * 60_000).toISOString(),
    binding: { protocol: 'mqtt', endpoint: 'mqtt://10.0.0.34:1883', node_id: 'cnc/12/state' },
  },
];

function rowToBinding(r: Record<string, unknown>): DeviceBinding {
  const pb = (r.protocol_binding ?? {}) as Record<string, unknown>;
  return {
    machineId: String(r.id ?? ''),
    code: String(r.code ?? ''),
    name: String(r.name ?? ''),
    status: String(r.status ?? ''),
    protocol: pb.protocol == null ? null : String(pb.protocol),
    endpoint: pb.endpoint == null ? null : String(pb.endpoint),
    nodeId: pb.node_id == null ? null : String(pb.node_id),
    lastHeartbeat: r.last_heartbeat == null ? null : String(r.last_heartbeat),
    binding: pb,
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
        .select('id,code,name,status,protocol_binding,last_heartbeat')
        .order('code', { ascending: true });
      if (error) throw new Error(error.message);
      return (data ?? []).map(rowToBinding);
    },
    enabled: !supabase || tenantId.length > 0,
    refetchInterval: 30_000,
  });
}

/**
 * Create or update a machine binding via the machine_upsert RPC. RLS does
 * NOT grant clients direct INSERT/UPDATE on machines — this RPC enforces
 * devices:write, validates the protocol family, and writes an audit row.
 * Demo mode returns the input id (or a generated one) so the form can show
 * a success state without a backend.
 */
export function useUpsertMachine() {
  const qc = useQueryClient();
  const { session } = useSession();
  const tenantId = session?.tenantId ?? '';
  return useMutation({
    mutationFn: async (a: UpsertMachineInput): Promise<string> => {
      if (!supabase) return a.id ?? crypto.randomUUID();
      const { data, error } = await supabase.rpc('machine_upsert', {
        p_id: a.id ?? null,
        p_code: a.code,
        p_name: a.name,
        p_status: a.status,
        p_protocol: a.protocol,
        p_binding: a.binding ?? {},
      });
      if (error) throw new Error(error.message);
      return String(data);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['device-bindings', tenantId] });
      qc.invalidateQueries({ queryKey: ['machines', tenantId] });
    },
  });
}
