/**
 * Simulator drivers. Emit synthetic-but-believable samples so a customer can
 * verify the gateway → Supabase → web-app pipeline before wiring real PLCs.
 *
 * Pick a driver via protocol slug "sim-temperature" or "sim-counter" in
 * protocol_binding.protocol when registering a machine.
 */
import { registerDriver, type Driver, type DriverSample } from './registry.ts';
import type { MachineRow } from '../supabase.ts';

function simTemperature(machine: MachineRow): Driver {
  const base = Number(machine.protocol_binding?.base_c ?? 4);
  const drift = Number(machine.protocol_binding?.drift_amplitude ?? 0.4);
  let t = 0;
  return {
    async poll(): Promise<DriverSample[]> {
      t += 1;
      const v = base + Math.sin(t / 9) * drift + (Math.random() - 0.5) * 0.2;
      return Promise.resolve([{ tag: 'temperature_c', value: Number(v.toFixed(2)) }]);
    },
    async command(name: string): Promise<boolean> {
      return Promise.resolve(name === 'noop');
    },
  };
}

function simCounter(machine: MachineRow): Driver {
  const step = Number(machine.protocol_binding?.step ?? 1);
  let n = 0;
  return {
    async poll(): Promise<DriverSample[]> {
      n += step;
      return Promise.resolve([{ tag: 'counter', value: n }]);
    },
    async command(name: string, payload: Record<string, unknown> | null): Promise<boolean> {
      if (name === 'reset') {
        n = Number(payload?.to ?? 0);
        return Promise.resolve(true);
      }
      return Promise.resolve(false);
    },
  };
}

export function registerSimulators(): void {
  registerDriver('sim-temperature', simTemperature);
  registerDriver('sim-counter', simCounter);
}
