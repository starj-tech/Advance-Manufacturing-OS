/**
 * Protocol driver registry. Each driver knows how to read a value (poll) and
 * execute a command for a single machine. The web app's binding form (V120)
 * writes the JSON config that lands in `machines.protocol_binding`; the
 * driver here consumes that same JSON.
 *
 * Real OPC-UA / MQTT / Modbus implementations are deferred — they each need
 * a dedicated Deno/npm client library. The shape here is enough to plug them
 * in without restructuring the loop.
 */
import type { MachineRow } from '../supabase.ts';

export interface DriverSample {
  tag: string;
  value: number | string | boolean | null;
}

export interface Driver {
  /** Single read pass. May emit zero, one, or many samples per call. */
  poll(): Promise<DriverSample[]>;
  /** Apply a command. Returns true if accepted. */
  command(name: string, payload: Record<string, unknown> | null): Promise<boolean>;
  /** Optional cleanup when the gateway is shutting down. */
  close?(): Promise<void> | void;
}

export type DriverFactory = (machine: MachineRow) => Driver;

const REGISTRY = new Map<string, DriverFactory>();

export function registerDriver(protocol: string, factory: DriverFactory): void {
  REGISTRY.set(protocol, factory);
}

export function driverFor(machine: MachineRow): Driver | null {
  const protocol = String(machine.protocol_binding?.protocol ?? '');
  const factory = REGISTRY.get(protocol);
  return factory ? factory(machine) : null;
}

export function supportedProtocols(): string[] {
  return [...REGISTRY.keys()].sort();
}
