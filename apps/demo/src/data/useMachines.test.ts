import { describe, it, expect } from 'vitest';
import { Machine } from '@aether/rpc-contracts';
import { rowToMachine } from './mappers';
import { DEMO_MACHINES } from './demo/machines';

describe('machines data layer', () => {
  it('demo machines satisfy the Machine contract', () => {
    expect(DEMO_MACHINES.length).toBeGreaterThan(0);
    expect(() => Machine.array().parse(DEMO_MACHINES)).not.toThrow();
  });

  it('maps a snake_case DB row into a valid Machine', () => {
    const row = {
      id: '0a000000-0000-4000-8000-0000000000ff',
      tenant_id: '00000000-0000-0000-0000-000000000001',
      code: 'LATHE-09',
      name: 'CNC lathe 9',
      protocol_binding: null,
      status: 'paused',
      last_heartbeat: null,
      hlc: '7.3.nodeA',
    };
    const m = Machine.parse(rowToMachine(row));
    expect(m.code).toBe('LATHE-09');
    expect(m.status).toBe('paused');
    expect(m.tenantId).toBe('00000000-0000-0000-0000-000000000001');
  });

  it('rejects a row with an invalid status', () => {
    const bad = {
      id: '0a000000-0000-4000-8000-0000000000fe',
      tenant_id: '00000000-0000-0000-0000-000000000001',
      code: 'X',
      name: 'X',
      protocol_binding: null,
      status: 'exploded',
      last_heartbeat: null,
      hlc: '1.0.n',
    };
    expect(() => Machine.parse(rowToMachine(bad))).toThrow();
  });
});
