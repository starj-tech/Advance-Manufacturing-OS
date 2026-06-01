/**
 * Modbus TCP driver — adapts the wire-level client (modbus-tcp.ts) to the
 * Driver interface consumed by the main loop. Connection is opened lazily on
 * first poll and reused; reconnect on any read error.
 *
 * Binding shape (set in the IT app's MachineBindingDialog):
 *   protocol_binding.protocol     = 'modbus-tcp'
 *   protocol_binding.endpoint     = '10.0.0.50:502'
 *   protocol_binding.unit_id      = '1'
 *   protocol_binding.register     = '40001'  // Modicon or 0-based
 *   protocol_binding.count        = '4'      // optional, default 1
 *   protocol_binding.tag_prefix   = 'press'  // optional
 */
import { connect, parseRegisterAddress, type ModbusConnection } from './modbus-tcp.ts';
import { registerDriver, type Driver, type DriverSample } from './registry.ts';
import type { MachineRow } from '../supabase.ts';

function modbusTcp(machine: MachineRow): Driver {
  const b = machine.protocol_binding ?? {};
  const endpoint = String(b.endpoint ?? '');
  const [host, portStr] = endpoint.split(':');
  const port = Number(portStr) || 502;
  const unitId = Number(b.unit_id ?? 1);
  const addr = parseRegisterAddress(String(b.register ?? '40001'));
  const count = Math.max(1, Number(b.count ?? 1));
  const tagPrefix = String(b.tag_prefix ?? 'holding');

  let conn: ModbusConnection | null = null;
  let connecting: Promise<ModbusConnection> | null = null;

  async function ensureConn(): Promise<ModbusConnection> {
    if (conn) return conn;
    if (connecting) return connecting;
    connecting = connect({ host, port, unitId }).then((c) => {
      conn = c;
      connecting = null;
      return c;
    });
    return connecting;
  }

  return {
    async poll(): Promise<DriverSample[]> {
      try {
        const c = await ensureConn();
        const regs = await c.read(addr.kind, addr.address, count);
        return regs.map((value, i) => ({ tag: `${tagPrefix}_${i}`, value }));
      } catch (e) {
        // Drop connection so the next poll reconnects.
        conn?.close();
        conn = null;
        throw e;
      }
    },
    async command(name: string): Promise<boolean> {
      // Write paths (FC 6, 16) not implemented yet — explicit refusal so the
      // ack lands with a clear detail string.
      return Promise.resolve(name === 'noop');
    },
    close() {
      conn?.close();
      conn = null;
    },
  };
}

export function registerModbusTcp(): void {
  registerDriver('modbus-tcp', modbusTcp);
}
