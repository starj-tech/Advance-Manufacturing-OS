import { assertEquals, assertGreater } from 'jsr:@std/assert';
import { tick, heartbeatTick, type Deps } from './loop.ts';
import { registerSimulators } from './drivers/simulator.ts';
import { driverFor } from './drivers/registry.ts';
import type { GatewayConfig } from './config.ts';
import type { MachineRow, CommandRow, SamplePayload } from './supabase.ts';

registerSimulators();

const cfg: GatewayConfig = {
  supabaseUrl: 'http://test',
  supabaseKey: 'test',
  tenantId: 't1',
  gatewayId: 'gw-test',
  pollIntervalMs: 5000,
  heartbeatIntervalMs: 30000,
  simulate: true,
};

const machines: MachineRow[] = [
  { id: 'm1', code: 'SIM-1', protocol_binding: { protocol: 'sim-counter', step: 5 } },
  { id: 'm2', code: 'SIM-2', protocol_binding: { protocol: 'sim-temperature' } },
  { id: 'm3', code: 'UNK', protocol_binding: { protocol: 'opcua' } }, // no driver
];

function makeDeps(): Deps & {
  samples: SamplePayload[];
  acked: Array<{ id: string; ok: boolean }>;
  heartbeats: string[];
} {
  const samples: SamplePayload[] = [];
  const acked: Array<{ id: string; ok: boolean }> = [];
  const heartbeats: string[] = [];
  return {
    samples,
    acked,
    heartbeats,
    listMachines: () => Promise.resolve(machines),
    pullCommands: () => Promise.resolve([] as CommandRow[]),
    ackCommand: (id, ok) => {
      acked.push({ id, ok });
      return Promise.resolve();
    },
    pushSample: (s) => {
      samples.push(s);
      return Promise.resolve();
    },
    heartbeat: (id) => {
      heartbeats.push(id);
      return Promise.resolve();
    },
    driverFor,
    log: () => {},
    now: () => '2026-01-01T00:00:00Z',
  };
}

Deno.test('tick polls every machine that has a driver and skips unsupported ones', async () => {
  const deps = makeDeps();
  const result = await tick(cfg, deps);
  // Two simulator machines → at least one sample each.
  assertGreater(result.samples, 1);
  assertEquals(result.skipped, 1);
  // Counter sample should be 5 (step=5).
  const counter = deps.samples.find((s) => s.machine_id === 'm1');
  assertEquals(counter?.tag, 'counter');
  assertEquals(counter?.value, 5);
});

Deno.test('tick acks pending commands per driver', async () => {
  const deps = makeDeps();
  const cmds: CommandRow[] = [
    { id: 'c1', machine_id: 'm1', command: 'reset', payload: { to: 100 } },
    { id: 'c2', machine_id: 'm3', command: 'whatever', payload: null }, // no driver
  ];
  deps.pullCommands = () => Promise.resolve(cmds);
  await tick(cfg, deps);
  const c1 = deps.acked.find((a) => a.id === 'c1');
  const c2 = deps.acked.find((a) => a.id === 'c2');
  assertEquals(c1?.ok, true);
  assertEquals(c2?.ok, false);
});

Deno.test('heartbeatTick pings only machines with a driver', async () => {
  const deps = makeDeps();
  const pinged = await heartbeatTick(deps);
  assertEquals(pinged, 2);
  assertEquals(deps.heartbeats.sort(), ['m1', 'm2']);
});
