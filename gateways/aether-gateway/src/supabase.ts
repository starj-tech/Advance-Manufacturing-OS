/**
 * Tiny Supabase REST client. Avoids the official supabase-js bundle so the
 * compiled gateway binary stays small (deno compile + supabase-js ≈ 60 MB;
 * this hand-rolled version + Deno runtime ≈ 30 MB).
 */
import type { GatewayConfig } from './config.ts';

export interface MachineRow {
  id: string;
  code: string;
  protocol_binding: Record<string, unknown> | null;
}

export interface CommandRow {
  id: string;
  machine_id: string;
  command: string;
  payload: Record<string, unknown> | null;
}

export interface SamplePayload {
  tenant_id: string;
  machine_id: string;
  tag: string;
  value: number | string | boolean | null;
  taken_at: string;
  gateway_id: string;
}

async function request(
  cfg: GatewayConfig,
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  return fetch(`${cfg.supabaseUrl}${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      apikey: cfg.supabaseKey,
      authorization: `Bearer ${cfg.supabaseKey}`,
      ...(init.headers ?? {}),
    },
  });
}

/** Machines configured for this tenant; the gateway connects to those it can. */
export async function listMachines(cfg: GatewayConfig): Promise<MachineRow[]> {
  const res = await request(
    cfg,
    `/rest/v1/machines?tenant_id=eq.${cfg.tenantId}&select=id,code,protocol_binding`,
  );
  if (!res.ok) throw new Error(`listMachines: ${res.status}`);
  return (await res.json()) as MachineRow[];
}

/** Pending commands queued by the web app for this tenant. */
export async function pullCommands(cfg: GatewayConfig): Promise<CommandRow[]> {
  const res = await request(
    cfg,
    `/rest/v1/actuator_commands?tenant_id=eq.${cfg.tenantId}&status=eq.pending&select=id,machine_id,command,payload&limit=50`,
  );
  if (!res.ok) return [];
  return (await res.json()) as CommandRow[];
}

/** Acknowledge a command after the driver attempted it. */
export async function ackCommand(
  cfg: GatewayConfig,
  id: string,
  ok: boolean,
  detail: string,
): Promise<void> {
  await request(cfg, `/rest/v1/actuator_commands?id=eq.${id}`, {
    method: 'PATCH',
    body: JSON.stringify({
      status: ok ? 'done' : 'failed',
      ack_detail: detail,
      acked_at: new Date().toISOString(),
    }),
  });
}

/** Push a single sample. */
export async function pushSample(cfg: GatewayConfig, s: SamplePayload): Promise<void> {
  await request(cfg, '/rest/v1/gateway_samples', {
    method: 'POST',
    body: JSON.stringify(s),
  });
}

/** Bump last_heartbeat so the IT app's DevicesPage shows this machine live. */
export async function heartbeat(cfg: GatewayConfig, machineId: string): Promise<void> {
  await request(cfg, `/rest/v1/machines?id=eq.${machineId}`, {
    method: 'PATCH',
    body: JSON.stringify({ last_heartbeat: new Date().toISOString() }),
  });
}
