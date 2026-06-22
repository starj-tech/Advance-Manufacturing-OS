import { Machine, WorkOrder } from '@aether/rpc-contracts';

const DEMO_TENANT = '00000000-0000-0000-0000-000000000001';

/** Demo machines (no backend) — self-validating against the Machine contract. */
export const DEMO_MACHINES = Machine.array().parse([
  {
    id: '0a000000-0000-4000-8000-000000000001',
    tenantId: DEMO_TENANT,
    code: 'PRESS-01',
    name: 'Hydraulic press 250t',
    protocolBinding: null,
    status: 'running',
    lastHeartbeat: null,
    hlc: '1.0.seed',
  },
  {
    id: '0a000000-0000-4000-8000-000000000002',
    tenantId: DEMO_TENANT,
    code: 'CNC-12',
    name: 'CNC mill A12',
    protocolBinding: null,
    status: 'idle',
    lastHeartbeat: null,
    hlc: '1.0.seed',
  },
  {
    id: '0a000000-0000-4000-8000-000000000003',
    tenantId: DEMO_TENANT,
    code: 'WELD-04',
    name: 'Robotic welder W4',
    protocolBinding: null,
    status: 'fault',
    lastHeartbeat: null,
    hlc: '1.0.seed',
  },
]);

/** Demo work orders (no backend) — self-validating against the WorkOrder contract. */
export const DEMO_WORK_ORDERS = WorkOrder.array().parse([
  {
    id: '0b000000-0000-4000-8000-000000000042',
    tenantId: DEMO_TENANT,
    code: 'WO-2026-0042',
    machineId: null,
    status: 'running',
    qtyPlanned: 150,
    qtyDone: 120,
    dueAt: null,
    createdBy: null,
    hlc: '1.0.seed',
    parentHlc: null,
  },
  {
    id: '0b000000-0000-4000-8000-000000000043',
    tenantId: DEMO_TENANT,
    code: 'WO-2026-0043',
    machineId: null,
    status: 'released',
    qtyPlanned: 80,
    qtyDone: 0,
    dueAt: null,
    createdBy: null,
    hlc: '1.0.seed',
    parentHlc: null,
  },
]);
