/**
 * Gateway configuration loaded from environment.
 *
 * Required env vars (set them in /etc/systemd/system/aether-gateway.service.d/
 * override.conf or in a .env file alongside the binary):
 *   AETHER_SUPABASE_URL          — https://<project>.supabase.co
 *   AETHER_SUPABASE_KEY          — service-role key (this binary runs on the
 *                                   plant's edge server and is trusted)
 *   AETHER_TENANT_ID             — tenant uuid
 *   AETHER_GATEWAY_ID            — short string identifying THIS install,
 *                                   eg. "plant-1-edge-01"
 * Optional:
 *   AETHER_POLL_INTERVAL_MS      — main loop interval, default 5000
 *   AETHER_HEARTBEAT_INTERVAL_MS — heartbeat tick, default 30000
 *   AETHER_SIMULATE              — "1" to enable the simulator driver (no
 *                                   real hardware required)
 */
export interface GatewayConfig {
  supabaseUrl: string;
  supabaseKey: string;
  tenantId: string;
  gatewayId: string;
  pollIntervalMs: number;
  heartbeatIntervalMs: number;
  simulate: boolean;
}

export class ConfigError extends Error {}

function envOrThrow(name: string): string {
  const v = Deno.env.get(name);
  if (!v || v.length === 0) throw new ConfigError(`missing env var ${name}`);
  return v;
}

function envInt(name: string, fallback: number): number {
  const v = Deno.env.get(name);
  if (!v) return fallback;
  const n = Number(v);
  if (!Number.isFinite(n) || n <= 0) throw new ConfigError(`invalid ${name}: ${v}`);
  return n;
}

export function loadConfig(): GatewayConfig {
  return {
    supabaseUrl: envOrThrow('AETHER_SUPABASE_URL').replace(/\/$/, ''),
    supabaseKey: envOrThrow('AETHER_SUPABASE_KEY'),
    tenantId: envOrThrow('AETHER_TENANT_ID'),
    gatewayId: envOrThrow('AETHER_GATEWAY_ID'),
    pollIntervalMs: envInt('AETHER_POLL_INTERVAL_MS', 5_000),
    heartbeatIntervalMs: envInt('AETHER_HEARTBEAT_INTERVAL_MS', 30_000),
    simulate: Deno.env.get('AETHER_SIMULATE') === '1',
  };
}
