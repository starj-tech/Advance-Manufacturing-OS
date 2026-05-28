import { Machine } from '@aether/rpc-contracts';

const DEMO_TENANT = '00000000-0000-0000-0000-000000000001';

/**
 * Machines shown when no Supabase backend is configured (demo mode). Parsed
 * through the Machine contract at module load so demo data can never drift
 * from the schema. Mirrors the four seeded machines in supabase/seed.sql.
 */
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
  {
    id: '0a000000-0000-4000-8000-000000000004',
    tenantId: DEMO_TENANT,
    code: 'PAINT-02',
    name: 'Powder coat line',
    protocolBinding: null,
    status: 'maintenance',
    lastHeartbeat: null,
    hlc: '1.0.seed',
  },
]);
