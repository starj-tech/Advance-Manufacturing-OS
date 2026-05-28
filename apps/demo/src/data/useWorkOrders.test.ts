import { describe, it, expect } from 'vitest';
import { WorkOrder } from '@aether/rpc-contracts';
import { rowToWorkOrder } from './mappers';
import { DEMO_WORK_ORDERS } from './demo/work-orders';

describe('work-orders data layer', () => {
  it('demo work orders satisfy the WorkOrder contract', () => {
    expect(DEMO_WORK_ORDERS.length).toBeGreaterThan(0);
    expect(() => WorkOrder.array().parse(DEMO_WORK_ORDERS)).not.toThrow();
  });

  it('coerces NUMERIC qty strings from PostgREST into numbers', () => {
    const row = {
      id: '0b000000-0000-4000-8000-0000000000aa',
      tenant_id: '00000000-0000-0000-0000-000000000001',
      code: 'WO-TEST-1',
      machine_id: null,
      status: 'running',
      qty_planned: '150.0000',
      qty_done: '120.5000',
      due_at: null,
      created_by: null,
      hlc: '9.1.nodeB',
      parent_hlc: null,
    };
    const wo = WorkOrder.parse(rowToWorkOrder(row));
    expect(wo.qtyPlanned).toBe(150);
    expect(wo.qtyDone).toBe(120.5);
    expect(wo.machineId).toBeNull();
  });
});
