/**
 * snake_case (PostgREST) → camelCase (rpc-contracts) row mappers. Each hook
 * validates the mapped object with its Zod schema, so this file owns only the
 * column lists + name translation. NUMERIC columns arrive as strings — coerce.
 */

export const MACHINE_COLUMNS = 'id,tenant_id,code,name,protocol_binding,status,last_heartbeat,hlc';

export function rowToMachine(r: Record<string, unknown>): Record<string, unknown> {
  return {
    id: r.id,
    tenantId: r.tenant_id,
    code: r.code,
    name: r.name,
    protocolBinding: r.protocol_binding ?? null,
    status: r.status,
    lastHeartbeat: r.last_heartbeat ?? null,
    hlc: r.hlc,
  };
}

export const WO_COLUMNS =
  'id,tenant_id,code,machine_id,status,qty_planned,qty_done,due_at,created_by,hlc,parent_hlc';

export function rowToWorkOrder(r: Record<string, unknown>): Record<string, unknown> {
  return {
    id: r.id,
    tenantId: r.tenant_id,
    code: r.code,
    machineId: r.machine_id ?? null,
    status: r.status,
    qtyPlanned: r.qty_planned == null ? 0 : Number(r.qty_planned),
    qtyDone: r.qty_done == null ? 0 : Number(r.qty_done),
    dueAt: r.due_at ?? null,
    createdBy: r.created_by ?? null,
    hlc: r.hlc,
    parentHlc: r.parent_hlc ?? null,
  };
}
