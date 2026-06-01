/**
 * Main gateway loop. Pure-ish: takes injected I/O so the unit tests in
 * loop.test.ts can verify behavior without hitting Supabase.
 */
import type { GatewayConfig } from './config.ts';
import type { CommandRow, MachineRow, SamplePayload } from './supabase.ts';
import type { Driver } from './drivers/registry.ts';

export interface Deps {
  listMachines: () => Promise<MachineRow[]>;
  pullCommands: () => Promise<CommandRow[]>;
  ackCommand: (id: string, ok: boolean, detail: string) => Promise<void>;
  pushSample: (s: SamplePayload) => Promise<void>;
  heartbeat: (machineId: string) => Promise<void>;
  driverFor: (machine: MachineRow) => Driver | null;
  log: (msg: string) => void;
  now: () => string;
}

/** Run one tick: refresh machine list, pull pending commands, poll each driver. */
export async function tick(
  cfg: GatewayConfig,
  deps: Deps,
): Promise<{
  samples: number;
  commands: number;
  skipped: number;
}> {
  const machines = await deps.listMachines();
  const drivers = new Map<string, Driver>();
  let skipped = 0;
  for (const m of machines) {
    const d = deps.driverFor(m);
    if (d) drivers.set(m.id, d);
    else skipped += 1;
  }

  // Apply pending commands (best-effort, ack each).
  const commands = await deps.pullCommands();
  for (const c of commands) {
    const d = drivers.get(c.machine_id);
    if (!d) {
      await deps.ackCommand(c.id, false, 'no driver for machine');
      continue;
    }
    try {
      const ok = await d.command(c.command, c.payload ?? null);
      await deps.ackCommand(c.id, ok, ok ? 'applied' : 'rejected');
    } catch (e) {
      await deps.ackCommand(c.id, false, (e as Error).message);
    }
  }

  // Poll drivers and push samples.
  let totalSamples = 0;
  for (const [machineId, driver] of drivers) {
    try {
      const samples = await driver.poll();
      for (const s of samples) {
        await deps.pushSample({
          tenant_id: cfg.tenantId,
          machine_id: machineId,
          tag: s.tag,
          value: s.value,
          taken_at: deps.now(),
          gateway_id: cfg.gatewayId,
        });
        totalSamples += 1;
      }
    } catch (e) {
      deps.log(`poll error ${machineId}: ${(e as Error).message}`);
    }
  }

  return { samples: totalSamples, commands: commands.length, skipped };
}

/** Heartbeat loop — emits an UPDATE to machines.last_heartbeat for every
 *  machine the gateway successfully has a driver for. */
export async function heartbeatTick(deps: Deps): Promise<number> {
  const machines = await deps.listMachines();
  let pinged = 0;
  for (const m of machines) {
    if (!deps.driverFor(m)) continue;
    await deps.heartbeat(m.id);
    pinged += 1;
  }
  return pinged;
}
