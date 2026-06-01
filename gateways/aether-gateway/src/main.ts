/**
 * AETHER-OS gateway daemon.
 *
 * Runs on an edge server on the customer's factory floor. Speaks industrial
 * protocols inbound to PLCs/sensors and outbound HTTPS to Supabase. Outbound-
 * only keeps it firewall-friendly: no ports need to be opened on the plant
 * side.
 *
 * Quick start:
 *   1. Install Deno: `curl -fsSL https://deno.land/install.sh | sh`
 *   2. Build:        `deno task build`
 *   3. Set env vars (see config.ts), then run `./dist/aether-gateway`.
 *
 * Simulator mode (no hardware required):
 *   AETHER_SIMULATE=1 ./dist/aether-gateway
 *   Register a machine in the web app (IT > Perangkat & Protokol > Tambah)
 *   with protocol = "sim-counter" or "sim-temperature".
 */
import { loadConfig, ConfigError } from './config.ts';
import { tick, heartbeatTick, type Deps } from './loop.ts';
import { listMachines, pullCommands, ackCommand, pushSample, heartbeat } from './supabase.ts';
import { driverFor, supportedProtocols } from './drivers/registry.ts';
import { registerSimulators } from './drivers/simulator.ts';
import { registerModbusTcp } from './drivers/modbus.ts';

function logInfo(msg: string): void {
  console.log(`${new Date().toISOString()} INFO  ${msg}`);
}
function logErr(msg: string): void {
  console.error(`${new Date().toISOString()} ERROR ${msg}`);
}

async function main(): Promise<void> {
  let cfg;
  try {
    cfg = loadConfig();
  } catch (e) {
    if (e instanceof ConfigError) {
      logErr(`config: ${e.message}`);
      Deno.exit(2);
    }
    throw e;
  }

  registerSimulators();
  registerModbusTcp();
  // Future: registerOpcua(); registerMqtt(); registerModbusRtu();

  logInfo(`aether-gateway ${cfg.gatewayId} → ${cfg.supabaseUrl} (tenant ${cfg.tenantId})`);
  logInfo(`drivers: ${supportedProtocols().join(', ') || '(none)'}`);

  const deps: Deps = {
    listMachines: () => listMachines(cfg),
    pullCommands: () => pullCommands(cfg),
    ackCommand: (id, ok, detail) => ackCommand(cfg, id, ok, detail),
    pushSample: (s) => pushSample(cfg, s),
    heartbeat: (id) => heartbeat(cfg, id),
    driverFor,
    log: logErr,
    now: () => new Date().toISOString(),
  };

  const abort = new AbortController();
  const onSignal = () => {
    logInfo('shutting down');
    abort.abort();
  };
  Deno.addSignalListener('SIGINT', onSignal);
  Deno.addSignalListener('SIGTERM', onSignal);

  let lastHeartbeat = 0;
  while (!abort.signal.aborted) {
    try {
      const t0 = Date.now();
      const result = await tick(cfg, deps);
      logInfo(
        `tick: samples=${result.samples} commands=${result.commands} skipped=${result.skipped}`,
      );
      if (Date.now() - lastHeartbeat >= cfg.heartbeatIntervalMs) {
        const pinged = await heartbeatTick(deps);
        logInfo(`heartbeat: ${pinged} machines`);
        lastHeartbeat = Date.now();
      }
      const elapsed = Date.now() - t0;
      const wait = Math.max(0, cfg.pollIntervalMs - elapsed);
      await new Promise((r) => setTimeout(r, wait));
    } catch (e) {
      logErr(`loop: ${(e as Error).message}`);
      await new Promise((r) => setTimeout(r, cfg.pollIntervalMs));
    }
  }
}

if (import.meta.main) {
  void main();
}
