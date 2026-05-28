import { WorkOrder } from '@aether/rpc-contracts';

const DEMO_TENANT = '00000000-0000-0000-0000-000000000001';

/**
 * Work orders shown in demo mode (no backend). Parsed through the WorkOrder
 * contract at module load so demo data cannot drift from the schema.
 */
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
  {
    id: '0b000000-0000-4000-8000-000000000044',
    tenantId: DEMO_TENANT,
    code: 'WO-2026-0044',
    machineId: null,
    status: 'paused',
    qtyPlanned: 200,
    qtyDone: 40,
    dueAt: null,
    createdBy: null,
    hlc: '1.0.seed',
    parentHlc: null,
  },
  {
    id: '0b000000-0000-4000-8000-000000000045',
    tenantId: DEMO_TENANT,
    code: 'WO-2026-0045',
    machineId: null,
    status: 'draft',
    qtyPlanned: 200,
    qtyDone: 0,
    dueAt: null,
    createdBy: null,
    hlc: '1.0.seed',
    parentHlc: null,
  },
]);
